//! Which fab table graded this board, and who decided.
//!
//! `cargo test -p cypcb-cli --test the_board_says_which_fab_it_is_for`
//!
//! `--preset` carried `default_value = "jlcpcb"` on every command that checks,
//! routes, scores or watches, so a caller who asked for JLCPCB and a caller who
//! asked for nothing arrived identical. A board written for OSHPark was
//! measured against JLCPCB's table unless whoever ran the command remembered to
//! say otherwise, and nothing in the output looked wrong.
//!
//! Each case is read off the line `cypcb check` already prints - `N DRC
//! violation(s) against <preset>` - which is the checker naming the table it
//! used rather than this test inferring it from geometry.

use std::process::Command;

/// A board with one unconnected part, so the report is never empty, carrying
/// whatever the caller wants said about its fab.
fn design(fab_line: &str) -> String {
    format!(
        "version 1\n\n\
         board t {{\n    size 30mm x 30mm\n    layers 2\n{fab_line}}}\n\n\
         component R1 resistor \"0402\" {{\n    value \"10k\"\n    at 10mm, 15mm\n}}\n"
    )
}

/// Run `cypcb check` and hand back everything it said.
fn check(case: &str, source: &str, flag: Option<&str>) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-fab-{case}"));
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, source).expect("the board is written");

    let mut command = Command::new(env!("CARGO_BIN_EXE_cypcb"));
    command.arg("check");
    if let Some(preset) = flag {
        command.args(["--preset", preset]);
    }
    let output = command.arg(&board).output().expect("the binary runs");

    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn the_board_decides_when_the_flag_is_absent() {
    let report = check("design", &design("    fab oshpark\n"), None);
    assert!(
        report.contains("against oshpark_2layer"),
        "the board asked for OSHPark and nothing on the command line disagreed:\n{report}"
    );
}

#[test]
fn the_flag_overrides_the_board() {
    let report = check("flag", &design("    fab oshpark\n"), Some("pcbway"));
    assert!(
        report.contains("against pcbway_standard"),
        "a caller naming a fab is asking a question about that fab:\n{report}"
    );
}

#[test]
fn silence_on_both_sides_is_still_jlcpcb() {
    let report = check("silent", &design(""), None);
    assert!(
        report.contains("against jlcpcb_standard_2layer"),
        "the default this project has always had:\n{report}"
    );
}

/// A typo in a file and a typo on the command line are fixed in different
/// places, so the message has to say which happened.
#[test]
fn a_fab_nobody_has_heard_of_says_where_it_was_written() {
    let from_design = check("typo", &design("    fab jlpcb\n"), None);
    assert!(
        from_design.contains("board asks for fab 'jlpcb'"),
        "{from_design}"
    );
    assert!(
        from_design.contains("oshpark_2layer"),
        "the refusal has to list what this tool does have:\n{from_design}"
    );

    let from_flag = check("typoflag", &design(""), Some("jlpcb"));
    assert!(from_flag.contains("Unknown preset 'jlpcb'"), "{from_flag}");
}
