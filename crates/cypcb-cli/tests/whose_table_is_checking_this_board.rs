//! A board is out of spec. Whose spec?
//!
//! `cargo test -p cypcb-cli --test whose_table_is_checking_this_board`
//!
//! Seven of the eleven presets are a fabricator's own published capability
//! page, read and dated in the source beside each figure. Three are the IPC
//! classes - a design standard with no public page to link to, so every number
//! in those tables is this project's reading of a document it cannot cite a
//! line of. One, `prototype`, is this project's own choice and answers to
//! nobody's table.
//!
//! All eleven used to print the same way. A reader told a board is out of spec
//! deserves to know which of the three said so.

use std::process::Command;

const A_BOARD: &str = r#"version 1

board t {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 10mm
}
"#;

fn check_against(preset: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-provenance-{preset}"));
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, A_BOARD).expect("the board is written");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", "--preset", preset])
        .arg(&board)
        .output()
        .expect("the binary runs");

    String::from_utf8_lossy(&output.stderr).to_string()
}

/// IPC-2221 and IPC-6012 are not public documents, so the figures cannot be
/// checked against anything a reader can open.
#[test]
fn an_ipc_class_says_it_is_a_standard_and_not_a_fab() {
    for preset in ["ipc1", "ipc2", "ipc3"] {
        let report = check_against(preset);
        assert!(
            report.contains("is a design standard rather than a fabricator"),
            "{preset}:\n{report}"
        );
        assert!(
            report.contains("not a public document"),
            "the reason has to be in it, not just the label - {preset}:\n{report}"
        );
    }
}

/// `prototype` is deliberately looser than any house requires.
#[test]
fn the_prototype_table_says_it_belongs_to_this_tool() {
    let report = check_against("prototype");
    assert!(
        report.contains("is this tool's own table, not a fabricator's"),
        "{report}"
    );
}

/// A fab's own published page needs no apology, and adding one to every report
/// would teach a reader to skip the line that matters.
#[test]
fn a_published_table_carries_no_caveat() {
    for preset in ["jlcpcb", "pcbway", "oshpark"] {
        let report = check_against(preset);
        assert!(
            !report.contains("design standard rather than a fabricator"),
            "{preset} is a published capability page:\n{report}"
        );
        assert!(
            !report.contains("this tool's own table"),
            "{preset} is a published capability page:\n{report}"
        );
    }
}
