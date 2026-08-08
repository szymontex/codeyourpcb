//! Whose number is `0.25mm required`?
//!
//! `cargo test -p cypcb-cli --test a_number_the_fab_never_stated_says_so`
//!
//! Three assembly-side rules have no counterpart in a fab's routing table, so
//! when a preset does not state one the checker derives it: a via pad is the
//! drill plus two annular rings, silk clearance follows the silk width, and
//! courtyard clearance takes a conservative IPC-style value. Every preset but
//! `prototype` leaves all three unstated.
//!
//! A number this tool chose and a number the fab published read exactly the
//! same in a violation, and a person deciding whether to move a part deserves
//! to know which they are looking at.

use std::process::Command;

/// Two 0402s half a millimetre apart: their courtyards overlap.
const TOUCHING: &str = r#"version 1

board t {
    size 30mm x 30mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 15mm
}

component R2 resistor "0402" {
    value "10k"
    at 10.5mm, 15mm
}
"#;

fn check_with(preset: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-note-{preset}"));
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, TOUCHING).expect("the board is written");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", "--preset", preset])
        .arg(&board)
        .output()
        .expect("the binary runs");

    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn a_derived_rule_that_fires_says_it_is_derived() {
    let report = check_with("jlcpcb");

    assert!(
        report.contains("courtyard-clearance"),
        "the fixture is supposed to break this rule:\n{report}"
    );
    assert!(
        report.contains("does not state a courtyard clearance"),
        "the number came from this tool and the report does not say so:\n{report}"
    );
    assert!(
        report.contains("this tool's own value, not the fab's"),
        "the note has to say whose number it is:\n{report}"
    );
}

#[test]
fn a_rule_the_preset_states_is_left_alone() {
    // `prototype` states a courtyard clearance of its own - 0.5mm, for hand
    // assembly - so the same violation needs no note. Without this the test
    // above passes on a note printed unconditionally, which would be noise on
    // every run.
    let report = check_with("prototype");

    assert!(
        report.contains("courtyard-clearance"),
        "the fixture breaks this rule under every preset:\n{report}"
    );
    assert!(
        !report.contains("does not state"),
        "prototype states this rule, so there is nothing to warn about:\n{report}"
    );
}

#[test]
fn a_rule_that_did_not_fire_is_not_mentioned() {
    // Silk clearance and via diameter are derived under `jlcpcb` too. Nothing
    // on this board breaks either, and a note about a rule nothing broke is a
    // line nobody reads.
    let report = check_with("jlcpcb");

    assert!(
        !report.contains("does not state a via diameter"),
        "{report}"
    );
    assert!(
        !report.contains("does not state a silkscreen clearance"),
        "{report}"
    );
}
