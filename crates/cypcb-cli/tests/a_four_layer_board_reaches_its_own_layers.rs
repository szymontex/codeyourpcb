//! Inner copper, from the file a person writes to the file a fabricator reads.
//!
//! `cargo test -p cypcb-cli --test a_four_layer_board_reaches_its_own_layers`
//!
//! Two defects hid behind each other here until 2026-08-08, and both needed
//! the whole chain to be visible:
//!
//! - a drilled pad was given `[TopCopper, BottomCopper]` whatever the board
//!   declared, so on a four-layer board it did not exist on In1 or In2 and no
//!   inner trace could reach it;
//! - the project numbers inner layers from zero - `job.rs` writes
//!   `Layer::Inner(n)` as `In{n + 1}` - while the KiCad reader read `In1.Cu`
//!   as `Inner(1)` and the KiCad writer wrote `Inner(n)` as `In{n}`.
//!
//! The unit tests each side of that could pass while the chain was broken,
//! because no test ran a four-layer `.cypcb` through routing and export and
//! looked at which Gerber the copper landed in. This one does.

use std::path::PathBuf;
use std::process::Command;

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

/// Route `four-layer.cypcb` and export it, returning the export directory.
///
/// Named per test: these run in parallel, and one directory shared between
/// them is one test deleting the files another is reading. That mistake has
/// been made three times in this repository now, each time looking like a
/// failure of the thing under test rather than of the harness.
fn routed_and_exported(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-four-layer-chain-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let routed = dir.join("four-layer.routed.cypcb");

    let route = cypcb()
        .arg("route")
        .arg("--in-house")
        .arg(repo_root().join("examples/four-layer.cypcb"))
        .arg("-o")
        .arg(&routed)
        .output()
        .expect("the binary runs");
    assert!(
        route.status.success(),
        "routing failed:\n{}",
        String::from_utf8_lossy(&route.stderr)
    );

    let export = cypcb()
        .arg("export")
        .arg(&routed)
        .arg("-o")
        .arg(&dir)
        .arg("--preset")
        .arg("jlcpcb")
        .output()
        .expect("the binary runs");
    // No `--force`. A board that cannot be exported without it is a board that
    // cannot be sent, and this example could not be until the drilled pads
    // reached their layers.
    assert!(
        export.status.success(),
        "exporting failed - if this is a short, the board is not routable as \
         drawn:\n{}",
        String::from_utf8_lossy(&export.stderr)
    );

    dir
}

fn layer(dir: &std::path::Path, name: &str) -> String {
    let path = dir
        .join("gerber")
        .join(format!("four-layer.routed-{name}.gbr"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn draws(gerber: &str) -> usize {
    gerber.lines().filter(|line| line.contains("D01")).count()
}

fn flashes(gerber: &str) -> usize {
    gerber.lines().filter(|line| line.contains("D03")).count()
}

#[test]
fn a_four_layer_board_exports_four_copper_layers() {
    let dir = routed_and_exported("files");

    for name in ["F_Cu", "In1_Cu", "In2_Cu", "B_Cu"] {
        let gerber = layer(&dir, name);
        assert!(
            gerber.contains("%FSLAX26Y26*%"),
            "{name} is not a Gerber file"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_drilled_pad_appears_on_every_copper_layer() {
    // The defect this catches: a through-hole pad given only the outer two
    // layers. Its copper has to be on all four, because the hole goes through
    // all four - and an inner trace has nothing to connect to otherwise.
    let dir = routed_and_exported("pads");

    for name in ["F_Cu", "In1_Cu", "In2_Cu", "B_Cu"] {
        let gerber = layer(&dir, name);
        // At least four, not exactly four: a via flashes its ring on the
        // layers it spans, and the router is free to place one.
        assert!(
            flashes(&gerber) >= 4,
            "{name} carries {} flashes, and the board has four drilled pads \
             that every copper layer passes through",
            flashes(&gerber)
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_inner_trace_is_on_the_inner_layer_the_file_named() {
    // `four-layer.cypcb` places one trace by hand, on Inner1. The DSL reads
    // that as `Layer::Inner(0)` and the exporter names it `In1_Cu`. Reading it
    // as `Inner(1)` instead - which the KiCad importer did - would put it in
    // `In2_Cu`, one layer deeper than the file says.
    let dir = routed_and_exported("inner");

    let in1 = layer(&dir, "In1_Cu");
    let in2 = layer(&dir, "In2_Cu");

    assert!(
        draws(&in1) > 0,
        "In1 carries no copper, and the file puts a trace there:\n{in1}"
    );
    assert_eq!(
        draws(&in2),
        0,
        "In2 carries copper the file never put there - the layer numbering is \
         off by one"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
