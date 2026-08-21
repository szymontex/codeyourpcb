//! A four-layer board's holes, split across the passes that drill them.
//!
//! `cargo test -p cypcb-cli --test every_hole_is_drilled_in_one_pass`
//!
//! A blind or buried via joins layers the through file cannot describe, so the
//! exporter writes one drill file per layer pair. Nothing had ever checked
//! that split on a board the router actually produced: the only coverage was
//! `a_buried_via_is_not_drilled_from_the_outside`, which builds one via by
//! hand.
//!
//! Measured on `multi_ic` routed by the in-house router: 119 vias, of which
//! **46 are blind or buried** across four layer pairs - including one buried
//! `In2..In1` that touches neither face. The export writes five drill files
//! and the hits sum to exactly the board's drilled pads plus its vias.
//!
//! Fixed counts are not asserted - they move whenever routing moves, and a
//! test that fails for that reason teaches people to update it without reading
//! it. What is asserted holds however the board is routed: the passes name
//! layer pairs the board has, and between them they drill each hole once.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// A drill directory and its files, each with the hits it carries.
type DrillFiles = (PathBuf, Vec<(String, Vec<String>)>);

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Route the four-layer fixture and export it, returning the drill directory.
///
/// **Routed once for the whole file.** Both tests here read one board, and
/// routing `multi_ic` in a debug build is the most expensive thing the test
/// suite does: this file took **24.20s of the workspace's 47.6s** - half the
/// wall clock of every test in the project - because each test routed and
/// exported the same fixture from scratch.
///
/// The hazard that made it per-test is real and is not this: two tests each
/// creating *and deleting* one shared directory means one wipes the files the
/// other is reading, and it presents as a failure of the thing under test. It
/// cost a full diagnosis here, for the fourth time in this repository. What is
/// safe is building the directory exactly once and never removing it again,
/// which is what `OnceLock` gives - the second test through waits for the
/// first and reads the same finished files.
///
/// The name carries the process id so two `cargo test` runs at once do not
/// meet in the same place, and neither test removes the directory when it
/// finishes - the one that finished first would be deleting what the other
/// is still reading. The leftovers are cleared by the next run that claims
/// the same name.
fn drill_files() -> &'static DrillFiles {
    static ROUTED: OnceLock<DrillFiles> = OnceLock::new();
    ROUTED.get_or_init(route_and_export)
}

fn route_and_export() -> DrillFiles {
    let dir = std::env::temp_dir().join(format!("cypcb-drill-passes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let board = dir.join("multi_ic.kicad_pcb");
    std::fs::copy(
        repo_root().join("tests/fixtures/benchmark/multi_ic.kicad_pcb"),
        &board,
    )
    .expect("the fixture is copyable");

    // `--fast`: one routing run instead of the best of eight. What is under
    // test is the drill split, which holds however the board is routed, and
    // best-of-eight costs eight times the wall clock for a board this size.
    let route = cypcb()
        .arg("route")
        .arg(&board)
        .arg("--fast")
        .output()
        .expect("the binary runs");
    assert!(
        route.status.success(),
        "routing failed:\n{}",
        String::from_utf8_lossy(&route.stderr)
    );

    // `--force`: this fixture routes with shorts, and the export guard refuses
    // a shorted board. What is under test here is the drill split, not whether
    // the board is fit to send.
    let export = cypcb()
        .arg("export")
        .arg(dir.join("multi_ic.routed.kicad_pcb"))
        .arg("-o")
        .arg(&dir)
        .arg("--house")
        .arg("jlcpcb")
        .arg("--force")
        .output()
        .expect("the binary runs");
    assert!(
        export.status.success(),
        "exporting failed:\n{}",
        String::from_utf8_lossy(&export.stderr)
    );

    let mut files: Vec<(String, Vec<String>)> = std::fs::read_dir(dir.join("drill"))
        .expect("a drill directory")
        .filter_map(|e| e.ok())
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let body = std::fs::read_to_string(entry.path()).expect("a readable drill file");
            let hits: Vec<String> = body
                .lines()
                .filter(|line| line.starts_with('X'))
                .map(|line| line.to_string())
                .collect();
            (name, hits)
        })
        .collect();
    files.sort();
    (dir, files)
}

#[test]
fn a_four_layer_board_gets_a_drill_file_per_layer_pair() {
    let (_, files) = drill_files();

    let through = files
        .iter()
        .find(|(name, _)| name.ends_with("-PTH.drl"))
        .expect("a plated through-hole file");
    assert!(
        !through.1.is_empty(),
        "the through file carries no holes at all"
    );

    let spans: Vec<&String> = files
        .iter()
        .map(|(name, _)| name)
        .filter(|name| name.contains("-PTH-"))
        .collect();
    assert!(
        !spans.is_empty(),
        "the router placed blind or buried vias and no span file was written: {:?}",
        files.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );

    // Every span file names a pair of layers this board has. `In3` on a
    // four-layer board would mean the naming and the stack disagree.
    for name in &spans {
        let pair = name
            .split("-PTH-")
            .nth(1)
            .and_then(|rest| rest.strip_suffix(".drl"))
            .unwrap_or_else(|| panic!("{name} is not named for a layer pair"));
        for layer in pair.split('-') {
            assert!(
                matches!(layer, "Top" | "Bottom" | "In1" | "In2"),
                "{name} names {layer}, which a four-layer board does not have"
            );
        }
    }
}

#[test]
fn the_passes_between_them_drill_the_board_exactly_once() {
    // The invariant that is actually checkable from the files.
    //
    // Asking "is any coordinate in two files" cannot distinguish a via written
    // twice - an export defect - from two different vias the router stacked at
    // one point, which is a board fault `cypcb check` reports as hole-to-hole.
    // Measured on this board: two coordinates appear in both the `Top-In1`
    // pass and the through pass, and the checker names one of them
    // `USB_DP <-> OSC_IN` at 0.00mm. Two vias, one point, two correct files.
    //
    // Counting is decisive where matching is not. Every drilled feature is
    // drilled once, so the hits across every pass sum to the board's drilled
    // pads plus its vias - 30 and 119 when this was written, 149 hits over
    // five files. A via written into two passes pushes the total above it.
    let (dir, files) = drill_files();

    let routed = std::fs::read_to_string(dir.join("multi_ic.routed.kicad_pcb"))
        .expect("the routed board is readable");
    let vias = routed
        .lines()
        .filter(|line| line.trim_start().starts_with("(via"))
        .count();
    let drilled_pads = routed
        .lines()
        .filter(|line| line.trim_start().starts_with("(pad") && line.contains("(drill "))
        .count();
    assert!(
        vias > 0 && drilled_pads > 0,
        "this board is supposed to have both vias and drilled pads: {vias} and {drilled_pads}"
    );

    let hits: usize = files.iter().map(|(_, hits)| hits.len()).sum();
    assert_eq!(
        hits,
        vias + drilled_pads,
        "{} hits across {} files against {vias} vias and {drilled_pads} drilled \
         pads - a hole drilled by two passes is a hole drilled twice",
        hits,
        files.len()
    );
}
