//! A net that states a width has stated a rule, and the checker holds the
//! trace to it.
//!
//! `cargo test -p cypcb-cli --test a_stated_width_is_a_rule`
//!
//! `net POWER [width 0.5mm]` reached the router - `ruleset_for_world` raises
//! `min_trace_width` for that net before a segment is drawn - and stopped
//! there. `MinTraceWidthRule` read the fab table alone, so a 0.2mm trace on a
//! net asking for 0.5mm passed: measured before the fix, the board below and
//! the same board with the statement deleted both reported **3 violations**,
//! and neither mentioned width. The same design, routed and checked,
//! disagreed with itself about the same sentence.
//!
//! The floor still wins where it is the wider of the two, which is the half a
//! rule that simply believed the net would get wrong.

use std::path::PathBuf;
use std::process::Command;

/// A net asking for more than its trace carries.
const WIDE_NET: &str = r#"version 1

board widths {
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

/// A net asking for less than JLCPCB's 0.127mm, on a trace that is narrower
/// still.
const NARROW_NET: &str = r#"version 1

board widths {
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

net A [width 0.05mm] {
    R1.1
    R2.1
}

trace A {
    from R1.1
    to R2.1
    layer Top
    width 0.1mm
}
"#;

/// One board, in a directory of this test's own: cargo runs the tests here at
/// the same time and a shared directory means one wiping what another is
/// reading.
fn board(who: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-stated-width-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, source).expect("the fixture is writable");
    board
}

fn run(command: &str, board: &PathBuf) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg(command)
        .arg(board)
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

/// What `score` says it found, out of the JSON it prints.
fn scored_violations(board: &PathBuf) -> u64 {
    let said = run("score", board);
    let start = said.find('{').expect("`score` prints an object");
    let end = said.rfind('}').expect("`score` prints an object");
    let json: serde_json::Value =
        serde_json::from_str(&said[start..=end]).expect("`score` prints JSON");
    json["drc_violations"]
        .as_u64()
        .expect("`score` counts DRC violations")
}

#[test]
fn a_trace_narrower_than_its_net_asked_for_is_a_violation() {
    let stated = board("wide", WIDE_NET);
    let said = run("check", &stated);
    assert!(
        said.contains("Trace width violation: 0.200mm actual, 0.500mm minimum"),
        "the net asked for 0.5mm and the trace carries 0.2mm:\n{said}"
    );

    // The same board with the statement deleted. 0.2mm is twice what JLCPCB
    // can etch, so without the sentence there is nothing to report - which is
    // what makes the sentence the thing being tested.
    let plain = board(
        "wide-plain",
        &WIDE_NET.replace("net A [width 0.5mm] {", "net A {"),
    );
    let floor = run("check", &plain);
    assert!(
        !floor.contains("trace-width"),
        "0.2mm clears the fab floor, so the statement is what this reports:\n{floor}"
    );
}

#[test]
fn a_net_cannot_ask_for_less_than_the_fab_can_etch() {
    // 0.05mm stated, 0.1mm drawn, 0.127mm etchable. A rule that believed the
    // net would call this board fine.
    let asked = board("narrow", NARROW_NET);
    let said = run("check", &asked);
    assert!(
        said.contains("Trace width violation: 0.100mm actual, 0.127mm minimum"),
        "the fab floor is the wider of the two and has to be what is \
         required:\n{said}"
    );
}

#[test]
fn score_counts_the_statement_too() {
    let stated = board("score-wide", WIDE_NET);
    let plain = board(
        "score-plain",
        &WIDE_NET.replace("net A [width 0.5mm] {", "net A {"),
    );

    let with = scored_violations(&stated);
    let without = scored_violations(&plain);
    assert_eq!(
        with,
        without + 1,
        "the statement is worth exactly the one trace that disobeys it: \
         {with} against {without}"
    );
}
