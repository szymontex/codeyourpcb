//! A net that states its own clearance is held to it, by both commands.
//!
//! `cargo test -p cypcb-cli --test a_stated_clearance_is_what_the_commands_check`
//!
//! `net HV [clearance 1mm]` is the design saying the fab's floor is not the
//! answer for this net. `ClearanceRule` reads it - and reads it twice, because
//! a stated clearance has to widen the **broad phase** as well as the
//! comparison: a pair 0.8mm apart is filtered out before anyone asks what it
//! required, if the search only reaches as far as the fab's 0.127mm.
//!
//! That is covered per-rule inside `cypcb-drc`. What is covered here is the
//! whole way through: the parser storing the constraint, `sync` putting it on
//! the net, and the two commands that publish DRC numbers both reporting
//! against the stated figure rather than the floor. The same board with the
//! statement deleted is checked beside it, so what the statement changed is
//! visible rather than asserted.

use std::path::PathBuf;
use std::process::Command;

/// Two nets running 1mm apart: 0.8mm of copper between the two traces.
///
/// Well inside JLCPCB's 0.127mm floor, well outside the 1mm net A asks for.
const STATED: &str = r#"version 1

board tight {
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

component R3 resistor "0402" {
    value "10k"
    at 5mm, 11mm
}

component R4 resistor "0402" {
    value "10k"
    at 25mm, 11mm
}

net A [clearance 1mm] {
    R1.1
    R2.1
}

net B {
    R3.1
    R4.1
}

trace A {
    from R1.1
    to R2.1
    layer Top
    width 0.2mm
}

trace B {
    from R3.1
    to R4.1
    layer Top
    width 0.2mm
}
"#;

/// The pair of boards, in a directory of this test's own.
///
/// Two tests here build the same fixtures, and cargo runs them at the same
/// time: sharing one directory means one test wiping it while the other is
/// reading what it just wrote.
fn boards(who: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("cypcb-stated-clearance-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let stated = dir.join("stated.cypcb");
    std::fs::write(&stated, STATED).expect("the fixture is writable");

    // The same board with the statement deleted, and nothing else touched.
    let plain_source = STATED.replace("net A [clearance 1mm] {", "net A {");
    assert_ne!(
        plain_source, STATED,
        "the statement has to be in the fixture"
    );
    let plain = dir.join("plain.cypcb");
    std::fs::write(&plain, plain_source).expect("the fixture is writable");

    (stated, plain)
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
fn check_measures_the_pair_against_what_the_net_asked_for() {
    let (stated, plain) = boards("check");

    let said = run("check", &stated);
    assert!(
        said.contains("trace 'A' ↔ trace 'B'") && said.contains("0.80mm actual, 1.00mm required"),
        "the two traces are 0.8mm apart and net A asked for 1mm, so the pair \
         has to be measured against 1mm:\n{said}"
    );

    let floor = run("check", &plain);
    assert!(
        !floor.contains("1.00mm required"),
        "with the statement deleted nothing may still require 1mm:\n{floor}"
    );
    assert!(
        !floor.contains("trace 'A' ↔ trace 'B'"),
        "and the pair the statement caught has to be quiet again, or the \
         fixture proves nothing:\n{floor}"
    );
}

#[test]
fn score_counts_the_same_board_against_the_same_statement() {
    let (stated, plain) = boards("score");

    let with = scored_violations(&stated);
    let without = scored_violations(&plain);
    assert!(
        with > without,
        "`score` builds its rules its own way, and a net that states a \
         clearance has to reach it too: {with} against {without}"
    );

    // And the number it publishes is the one `check` prints for that board.
    let checked = run("check", &stated);
    assert!(
        checked.contains(&format!("{with} DRC violation(s)")),
        "the two commands have to agree on the board with the statement:\n{checked}"
    );
}
