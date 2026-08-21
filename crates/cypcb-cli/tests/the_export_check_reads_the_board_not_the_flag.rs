//! `cypcb export` checks a board on the way out. Against whose rules?
//!
//! `cargo test -p cypcb-cli --test the_export_check_reads_the_board_not_the_flag`
//!
//! `--preset` means two different things in this binary. On `check`, `route`,
//! `score` and `watch` it names a **design-rule** table (what a house can etch)
//! and knows ten fabs. On `export` it names a **file convention** (what a house
//! wants the Gerbers called) and knows two. The pre-export check used to build
//! its rules from the export flag, so the two lists were being read as one: a
//! board written for OSHPark was measured against JLCPCB on the way out,
//! because `--preset oshpark` is refused by this command long before the check
//! runs.
//!
//! The board decides now, the same way it decides for `cypcb check`.

use std::process::Command;

/// Two traces 0.14mm apart, a gap JLCPCB images and OSHPark does not, plus the
/// parts they run between so the board is a board.
fn design(fab_line: &str) -> String {
    format!(
        "version 1\n\n\
         board t {{\n    size 20mm x 20mm\n    layers 2\n{fab_line}}}\n\n\
         component R1 resistor \"0402\" {{\n    value \"10k\"\n    at 5mm, 5mm\n}}\n\n\
         component R2 resistor \"0402\" {{\n    value \"10k\"\n    at 15mm, 5mm\n}}\n\n\
         net A {{\n    R1.1\n    R2.1\n}}\n\n\
         net B {{\n    R1.2\n    R2.2\n}}\n\n\
         trace A {{\n    layer Top\n    width 0.127mm\n    path 5mm,10mm -> 15mm,10mm\n}}\n\n\
         trace B {{\n    layer Top\n    width 0.127mm\n    path 5mm,10.267mm -> 15mm,10.267mm\n}}\n"
    )
}

/// How many violations `cypcb export` warned about on its way out.
fn violations_reported(case: &str, fab_line: &str, flag: &str) -> usize {
    let dir = std::env::temp_dir().join(format!("cypcb-export-fab-{case}"));
    std::fs::create_dir_all(&dir).expect("a place to work in");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, design(fab_line)).expect("the board is written");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["export", "--house", flag, "-o"])
        .arg(dir.join("out"))
        .arg(&board)
        .output()
        .expect("the binary runs");

    let report = String::from_utf8_lossy(&output.stderr).to_string();
    let count = report
        .split("exporting a board with ")
        .nth(1)
        .and_then(|tail| tail.split_whitespace().next())
        .and_then(|number| number.parse().ok());

    count.unwrap_or_else(|| panic!("no violation count in:\n{report}"))
}

/// The number is read off the warning the command already prints, so this is
/// the checker naming its own count rather than the test inferring one.
#[test]
fn the_board_decides_which_rules_the_export_check_uses() {
    let jlcpcb = violations_reported("silent", "", "jlcpcb");
    let oshpark = violations_reported("oshpark", "    fab oshpark\n", "jlcpcb");

    assert_eq!(jlcpcb, 4, "a board naming no fab is checked against JLCPCB");
    assert_eq!(
        oshpark, 7,
        "the same geometry against OSHPark, which does not image a 0.14mm gap"
    );
}

/// The export flag names file conventions and must not move the rule set.
///
/// `pcbway` is the one name both lists share, which is exactly where the old
/// behaviour was invisible: exporting with it quietly changed which rules the
/// board was measured against, and nobody had asked for that.
#[test]
fn the_export_preset_does_not_change_which_rules_apply() {
    let as_jlcpcb = violations_reported("conv-jlcpcb", "    fab oshpark\n", "jlcpcb");
    let as_pcbway = violations_reported("conv-pcbway", "    fab oshpark\n", "pcbway");

    assert_eq!(
        as_jlcpcb, as_pcbway,
        "the file-naming convention is not a design rule table"
    );
}
