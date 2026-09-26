//! `check` says the same thing about the same board every time it is run.
//!
//! `cargo test -p cypcb-cli --test a_check_says_the_same_each_time`
//!
//! bevy finds the tables a query reads through a hash map, and the hash is
//! seeded afresh in every process. A rule that walked a query took the parts
//! in a different order from one run to the next: `Y1 <-> U1` in one run was
//! `U1 <-> Y1` at another place in the next, and the spatial index built the
//! same way measured a contact from a different end. One process cannot show
//! it, because the seed is fixed for the life of the process, so this runs the
//! binary again and again and asks for the same bytes each time.

use std::process::Command;

/// Turned parts of different kinds crowded together. The kinds matter: a
/// value written as a quantity carries a component a quoted value does not,
/// so the parts sit in different tables, and the tables are what the hash
/// orders. Turned courtyards overlap, so there are pairs to name.
const CROWDED: &str = r#"version 1

board crowded {
    size 40mm x 40mm
    layers 2
}

component U1 ic "TQFP-32" {
    value "MCU"
    at 20mm, 20mm
    rotate 30
}

component C1 capacitor "0402" {
    value 100nF
    at 15mm, 20mm
    rotate 45
}

component C2 capacitor "0402" {
    value 100nF
    at 25mm, 20mm
    rotate 45
}

component R1 resistor "0603" {
    value "10k"
    at 20mm, 15mm
    rotate 60
}

component Y1 crystal "SOT-23" {
    value 8MHz
    at 20mm, 25mm
    rotate 135
}
"#;

/// Separate processes, each with its own hash seed. Before the fix the
/// output split about one run in three on this board, so sixteen identical
/// runs by chance are well under one in a hundred.
const RUNS: usize = 16;

#[test]
fn every_run_prints_the_same_report() {
    let dir = cypcb_fixtures::scratch_dir("cypcb-check-same-each-time");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, CROWDED).expect("the fixture is writable");
    let board = dir.holding(board);

    let run = || {
        let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
            .args(["check", "-o", "json"])
            .arg(&*board)
            .output()
            .expect("the binary runs");
        (output.status.code(), output.stdout, output.stderr)
    };

    let first = run();
    let report = String::from_utf8_lossy(&first.1);
    for kind in ["courtyard-clearance", "clearance"] {
        assert!(
            report.contains(&format!("\"kind\": \"{kind}\"")),
            "the board has to give {kind} rows to put in order:\n{report}"
        );
    }

    for n in 1..RUNS {
        let again = run();
        assert_eq!(again.0, first.0, "run {n} exits differently");
        assert!(
            again.1 == first.1,
            "run {n} prints another report:\n--- first\n{}\n--- run {n}\n{}",
            report,
            String::from_utf8_lossy(&again.1)
        );
        assert!(again.2 == first.2, "run {n} says something else on stderr");
    }
}
