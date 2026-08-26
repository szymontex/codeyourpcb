//! What a KiCad file loses from a rigid-flex build, and what that costs.
//!
//! `cargo test -p cypcb-cli --test the_build_a_kicad_file_cannot_carry`
//!
//! `examples/blind-via.cypcb` states two things a `.kicad_pcb` has no field
//! for: the drill spans the build makes - `drill Top to Bottom`, `drill Top to
//! Inner1` - and the fabricator whose table the board is graded against. Both
//! are announced when the file is written, and both sentences make a claim
//! about what happens next. Nothing checked either claim.
//!
//! Measured here end to end. The span list really is gone, and the house is
//! not: `to-kicad` writes the name into the `.kicad_pro` beside the board and
//! `from-kicad` reads it back, so the board comes home to its own table and
//! passes. Separate the pair - read the board with no project file beside it -
//! and the cost of losing a house shows: graded against the default table, the
//! two blind vias it was written for are refused.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A directory of this test's own: cargo runs tests side by side.
fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-kicad-build-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    dir
}

fn run(args: &[&str]) -> (String, String, bool) {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}

/// What `check -o json` counted, by kind, and the table it used.
fn checked(board: &Path) -> (String, BTreeMap<String, usize>) {
    let (said, _, _) = run(&["check", "-o", "json", board.to_str().expect("a path")]);
    let report: serde_json::Value =
        serde_json::from_str(said.trim()).expect("check prints JSON on stdout");
    let mut counts = BTreeMap::new();
    for (kind, count) in report["summary"].as_object().expect("a summary") {
        counts.insert(kind.clone(), count.as_u64().expect("a count") as usize);
    }
    (
        report["preset"].as_str().unwrap_or_default().to_string(),
        counts,
    )
}

#[test]
fn the_span_list_is_lost_the_house_is_not() {
    let dir = scratch("blind-via");
    let board = dir.join("board.kicad_pcb");

    let (_, said, ok) = run(&[
        "to-kicad",
        "examples/blind-via.cypcb",
        "-o",
        board.to_str().expect("a path"),
    ]);
    assert!(ok, "writing the KiCad board failed:\n{said}");
    assert!(
        said.contains("Top to Bottom, Top to Inner1"),
        "the spans the build drills are named when they are dropped:\n{said}"
    );
    assert!(
        said.contains("pcbway"),
        "so is the fabricator whose table the board is graded against:\n{said}"
    );

    // Back again. The build's span list is gone with the file that could not
    // carry it; the house is not, because it rides in the project file.
    let back = dir.join("back.cypcb");
    let (_, said, ok) = run(&[
        "from-kicad",
        board.to_str().expect("a path"),
        "-o",
        back.to_str().expect("a path"),
    ]);
    assert!(ok, "reading the KiCad board back failed:\n{said}");
    assert!(
        said.contains("checked against pcbway"),
        "the command says where the house came from:\n{said}"
    );
    let design = std::fs::read_to_string(&back).expect("the design was written");
    assert!(
        !design.contains("drill Top"),
        "the span list does not survive the trip:\n{design}"
    );
    assert!(design.contains("fab pcbway"), "the house does:\n{design}");

    // So the board comes home to its own table and passes: the span list is
    // gone, and with a house that drills blind vias there is nothing to hold
    // them to - which is what the first warning claims.
    let (preset, counts) = checked(&back);
    assert_eq!(preset, "pcbway_standard", "{counts:?}");
    assert!(counts.is_empty(), "{counts:?}");

    // Separate the pair and the cost shows. A board read without the project
    // file beside it is graded against the default table, which does not drill
    // blind vias, and the board has two.
    let alone_dir = scratch("blind-via-alone");
    let alone = alone_dir.join("alone.kicad_pcb");
    std::fs::copy(&board, &alone).expect("the board is copyable");
    let orphan = alone_dir.join("orphan.cypcb");
    let (_, said, ok) = run(&[
        "from-kicad",
        alone.to_str().expect("a path"),
        "-o",
        orphan.to_str().expect("a path"),
    ]);
    assert!(ok, "reading the lone board failed:\n{said}");
    let (preset, counts) = checked(&orphan);
    assert_eq!(preset, "jlcpcb_standard_4layer", "{counts:?}");
    assert_eq!(counts.get("via-span").copied(), Some(2), "{counts:?}");
}

/// The numbers in that project file are read too, and only mentioned when they
/// disagree with the table the board will be checked against.
///
/// `--preset` has written eight constraints into the `.kicad_pro` since it
/// existed and nothing ever read them back, so a project somebody set up by
/// hand to a tighter clearance than the house publishes came home saying
/// nothing about it.
#[test]
fn a_project_file_that_states_its_own_rules_is_read() {
    let dir = scratch("project-rules");
    let board = dir.join("board.kicad_pcb");
    let (_, said, ok) = run(&[
        "to-kicad",
        "examples/blind-via.cypcb",
        "-o",
        board.to_str().expect("a path"),
    ]);
    assert!(ok, "writing the KiCad board failed:\n{said}");

    // As written, the project file states the house's own numbers, so there is
    // nothing to report and the command stays quiet about them.
    let quiet = dir.join("quiet.cypcb");
    let (_, said, ok) = run(&[
        "from-kicad",
        board.to_str().expect("a path"),
        "-o",
        quiet.to_str().expect("a path"),
    ]);
    assert!(ok, "reading the board back failed:\n{said}");
    assert!(
        !said.contains("states rules this language cannot"),
        "a project file agreeing with the table is not news:\n{said}"
    );

    // Somebody tightens the clearance by hand, the way a person does when a
    // board is going to a house's advanced process.
    let project = board.with_extension("kicad_pro");
    let text = std::fs::read_to_string(&project).expect("the project file is there");
    let tightened = text.replace("\"min_clearance\": 0.1", "\"min_clearance\": 0.09");
    assert_ne!(tightened, text, "the fixture states a clearance to change");
    std::fs::write(&project, tightened).expect("the project file is writable");

    let back = dir.join("back.cypcb");
    let (_, said, ok) = run(&[
        "from-kicad",
        board.to_str().expect("a path"),
        "-o",
        back.to_str().expect("a path"),
    ]);
    assert!(ok, "reading the board back failed:\n{said}");
    assert!(
        said.contains("minimum clearance 0.090mm against 0.100mm"),
        "the figure somebody set and the figure the board is checked against:\n{said}"
    );
    assert!(
        said.contains("pcbway_standard"),
        "and which table that is:\n{said}"
    );
}

/// A board whose net asks for a width comes home still asking for it.
///
/// `net SIG [width 0.5mm ...]` is read by three rules, and a `.kicad_pcb`
/// carries a net's membership and nothing else - so a round trip used to
/// return a design whose nets asked for nothing and whose trace-width,
/// trace-current and impedance rules checked nothing, silently.
const ASKING: &str = r#"version 1

board asks {
    size 30mm x 20mm
    layers 2
    fab jlcpcb
}

component R1 resistor "0402" {
    value "10k"
    at 5mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 25mm, 10mm
}

net SIG [width 0.5mm clearance 0.3mm current 500mA] {
    R1.1
    R2.1
}

trace SIG {
    from R1.1
    to R2.1
    layer Top
    width 0.2mm
}
"#;

#[test]
fn what_a_net_asks_for_survives_the_trip() {
    let dir = scratch("net-asks");
    let design = dir.join("asks.cypcb");
    std::fs::write(&design, ASKING).expect("the fixture is writable");

    // The design holds its own trace to 0.5mm, and the trace carries 0.2mm.
    let (_, counts) = checked(&design);
    assert_eq!(counts.get("trace-width").copied(), Some(1), "{counts:?}");

    let board = dir.join("board.kicad_pcb");
    let (_, said, ok) = run(&[
        "to-kicad",
        design.to_str().expect("a path"),
        "-o",
        board.to_str().expect("a path"),
    ]);
    assert!(ok, "writing the KiCad board failed:\n{said}");
    assert!(
        said.contains("what 1 net(s) ask for (SIG)"),
        "the loss is announced by name:\n{said}"
    );

    let back = dir.join("back.cypcb");
    let (_, said, ok) = run(&[
        "from-kicad",
        board.to_str().expect("a path"),
        "-o",
        back.to_str().expect("a path"),
    ]);
    assert!(ok, "reading the board back failed:\n{said}");
    assert!(
        said.contains("What 1 net(s) ask for comes from"),
        "and recovered from the project file beside it:\n{said}"
    );

    let source = std::fs::read_to_string(&back).expect("the design came back");
    assert!(
        source.contains("width 0.500000mm") && source.contains("current 500mA"),
        "the figures are written back into the language:\n{source}"
    );

    // The half that matters: the rule checks again.
    let (_, counts) = checked(&back);
    assert_eq!(
        counts.get("trace-width").copied(),
        Some(1),
        "the restored width is a rule the checker reads: {counts:?}"
    );

    // Read the board with no project file beside it and the net asks for
    // nothing, so nothing holds the trace to anything.
    let alone_dir = scratch("net-asks-alone");
    let alone = alone_dir.join("alone.kicad_pcb");
    std::fs::copy(&board, &alone).expect("the board is copyable");
    let orphan = alone_dir.join("orphan.cypcb");
    let (_, said, ok) = run(&[
        "from-kicad",
        alone.to_str().expect("a path"),
        "-o",
        orphan.to_str().expect("a path"),
    ]);
    assert!(ok, "reading the lone board failed:\n{said}");
    let (_, counts) = checked(&orphan);
    assert_eq!(counts.get("trace-width").copied(), None, "{counts:?}");
}
