//! `check` answers a program, not only a person.
//!
//! `cargo test -p cypcb-cli --test a_program_can_read_the_check`
//!
//! The verdict was an exit code and the detail was prose on stderr, so a CI
//! job could fail a build on "some rule broke" and never on *which* rule.
//! `score` carries a violation count in JSON and says nothing about what
//! fired or where.
//!
//! `-o json` prints the rows the browser already receives - kind, place, what
//! was measured, what was required, and the line of the file the entity was
//! written on - on stdout, with the prose left where prose belongs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A net asking for 0.5mm and a trace carrying 0.2mm: a hand-worked pair, so
/// the numbers in the report can be checked rather than merely present.
const NARROW: &str = r#"version 1

board narrow {
    size 30mm x 30mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 5mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 25mm, 10mm
}

net A [width 0.5mm] {
    R1.1
    R2.1
}

trace A {
    from R1.1
    to R2.1
    layer Top
    width 0.2mm
}
"#;

/// A board with faults of four different depths, in an order the registry
/// does not rank: the trace is 0.100mm against a 0.127mm floor, which is a
/// milder fault than a 0.05mm drill against 0.3mm, and the registry emits the
/// trace first.
const RANKED: &str = r#"version 1

footprint TINY_DRILL {
    description "two holes narrower than the house drills"
    courtyard 4mm x 4mm
    pad 1 circle at 0mm, 0mm size 1.6mm x 1.6mm drill 0.05mm
    pad 2 circle at 2.54mm, 0mm size 1.6mm x 1.6mm drill 0.05mm
}

board ranked {
    size 30mm x 30mm
    layers 2
}

component J1 connector "TINY_DRILL" {
    value "header"
    at 5mm, 10mm
}

component J2 connector "TINY_DRILL" {
    value "header"
    at 25mm, 10mm
}

net A {
    J1.1
    J2.1
}

trace A {
    from J1.1
    to J2.1
    layer Top
    width 0.10mm
}
"#;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// One board, in a directory of this test's own.
fn board(who: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-check-json-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, source).expect("the fixture is writable");
    board
}

fn check(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs")
}

/// The whole of stdout, as JSON.
fn report(output: &Output) -> serde_json::Value {
    let said = String::from_utf8_lossy(&output.stdout).to_string();
    serde_json::from_str(said.trim())
        .unwrap_or_else(|error| panic!("stdout should be JSON and nothing else: {error}\n{said}"))
}

#[test]
fn a_broken_rule_is_named_measured_and_placed() {
    let file = board("narrow", NARROW);
    let output = check(&["-o", "json", file.to_str().expect("a path")]);
    let report = report(&output);

    assert_eq!(report["ok"], false, "the board breaks a rule: {report}");
    assert_eq!(report["checked"], true, "the checker ran: {report}");
    assert_eq!(
        report["summary"]["trace-width"], 1,
        "one trace is narrower than its net asked for: {report}"
    );

    let rows = report["violations"].as_array().expect("a list of rows");
    let width = rows
        .iter()
        .find(|row| row["kind"] == "trace-width")
        .unwrap_or_else(|| panic!("no trace-width row: {report}"));
    assert!(
        width["message"]
            .as_str()
            .is_some_and(|said| said.contains("0.200mm actual, 0.500mm minimum")),
        "the row carries the two figures: {report}"
    );
    assert_eq!(
        width["line"], 23,
        "the trace is written on line 23 of the fixture: {report}"
    );
    assert_eq!(
        width["actual_mm"], 0.2,
        "the two figures in that sentence are numbers too: {report}"
    );
    assert_eq!(
        width["required_mm"], 0.5,
        "and the one the net asked for: {report}"
    );

    // The clearance rule measures a distance and carries it as a number, so a
    // program can tell copper touching copper from a gap under spec without
    // reading a sentence. R1's pad and the trace leaving it are at 0.00mm
    // against JLCPCB's 0.127mm.
    let clearance = rows
        .iter()
        .find(|row| row["kind"] == "clearance")
        .unwrap_or_else(|| panic!("no clearance row: {report}"));
    assert_eq!(clearance["actual_mm"], 0.0, "{report}");
    assert_eq!(clearance["required_mm"], 0.127, "{report}");

    // The exit code is the half a shell script reads first, and it has to go
    // on saying what it said before the flag existed.
    assert_eq!(
        output.status.code(),
        Some(1),
        "a board that breaks a rule exits 1"
    );
}

#[test]
fn a_clean_board_says_so_in_the_same_shape() {
    let output = check(&["-o", "json", "examples/blind-via.cypcb"]);
    let report = report(&output);

    assert_eq!(report["ok"], true, "the example passes: {report}");
    assert_eq!(report["checked"], true, "the checker ran: {report}");
    assert_eq!(
        report["violations"].as_array().map(Vec::len),
        Some(0),
        "a clean board has no rows: {report}"
    );
    assert_eq!(
        report["preset"], "pcbway_standard",
        "the board's own fab decides the table: {report}"
    );
    assert_eq!(output.status.code(), Some(0), "a clean board exits 0");
}

#[test]
fn the_report_for_a_person_is_untouched() {
    let file = board("prose", NARROW);
    let output = check(&[file.to_str().expect("a path")]);

    assert!(
        String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "the prose report leaves stdout empty for a failing board"
    );
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains("Trace width violation: 0.200mm actual, 0.500mm minimum"),
        "the sentence a person reads is unchanged:\n{said}"
    );
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn the_worst_fault_is_the_first_row() {
    let file = board("ranked", RANKED);
    let output = check(&["-o", "json", file.to_str().expect("a path")]);
    let report = report(&output);
    let rows = report["violations"].as_array().expect("a list of rows");

    let kinds: Vec<&str> = rows
        .iter()
        .map(|row| row["kind"].as_str().expect("every row names its rule"))
        .collect();

    // Copper touching copper measures nothing at all, so it is the deepest
    // fault on the board whatever rule the registry ran first.
    assert_eq!(kinds[0], "clearance", "{kinds:?}");

    // The discriminator: unsorted, the 0.100mm trace comes out above the
    // 0.05mm drills, and it is the milder fault of the two.
    let first_drill = kinds
        .iter()
        .position(|kind| *kind == "drill-size")
        .expect("the drills are too small to make");
    let width = kinds
        .iter()
        .position(|kind| *kind == "trace-width")
        .expect("the trace is under the floor");
    assert!(
        first_drill < width,
        "a 0.05mm drill against 0.3mm is deeper than 0.100mm against 0.127mm: {kinds:?}"
    );

    // How far under its rule each row is, as the report itself states it.
    let depth = |row: &serde_json::Value| -> Option<f64> {
        let actual = row["actual_mm"].as_f64()?;
        let required = row["required_mm"].as_f64()?;
        (required > 0.0).then(|| (required - actual) / required)
    };
    let measured: Vec<f64> = rows.iter().filter_map(depth).collect();
    assert!(
        measured.len() >= 4,
        "this board has faults of several depths: {kinds:?}"
    );
    assert!(
        measured.windows(2).all(|pair| pair[0] >= pair[1] - 1e-9),
        "the rows run deepest first: {measured:?}"
    );

    // A fault with no distance in it - a pin nothing reaches - sorts to the
    // end rather than among the ones that were measured.
    let unmeasured = rows.iter().filter(|row| depth(row).is_none()).count();
    assert_eq!(unmeasured, 2, "two pins are unconnected: {kinds:?}");
    assert!(
        rows.iter()
            .rev()
            .take(unmeasured)
            .all(|row| depth(row).is_none()),
        "the unmeasured rows are the last ones: {kinds:?}"
    );
}
