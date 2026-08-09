//! A board is what it is, whatever it is called.
//!
//! `cargo test -p cypcb-cli --test a_board_is_read_as_what_it_is`
//!
//! Every command decided which reader to use from the file's extension. A
//! KiCad board saved as `board.cypcb` - which happens when somebody renames a
//! file, or when a tool writes one - went to the DSL reader, and the DSL
//! reader had a lot to say about it: measured on a routed benchmark board,
//! **1000 parse errors over 10,998 lines, in 520ms**. One mistake answered
//! with eleven thousand lines.
//!
//! An extension is a claim; the first line is the fact.

use std::process::Command;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark")
        .join(name)
}

fn check(path: &std::path::Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(path)
        .output()
        .expect("the binary runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn a_kicad_board_under_a_cypcb_name_is_still_a_kicad_board() {
    let dir = std::env::temp_dir().join("cypcb-misnamed");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let misnamed = dir.join("board.cypcb");
    std::fs::copy(fixture("led_blink.kicad_pcb"), &misnamed).expect("the fixture is there");

    let report = check(&misnamed);

    assert!(
        !report.contains("cypcb::parse"),
        "it was read by the wrong reader:\n{}",
        report.lines().take(8).collect::<Vec<_>>().join("\n")
    );
    assert!(
        report.contains("DRC violation") || report.contains("passed DRC"),
        "a board was read, so it was checked:\n{report}"
    );
    assert!(
        report.lines().count() < 200,
        "one mistake, {} lines of answer",
        report.lines().count()
    );
}

#[test]
fn a_design_that_is_really_broken_still_says_so() {
    // The other direction: content sniffing must not swallow a genuine parse
    // error in a real `.cypcb`.
    let dir = std::env::temp_dir().join("cypcb-misnamed");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let broken = dir.join("broken.cypcb");
    std::fs::write(
        &broken,
        "version 1\n\nboard b {\n    size 20mm x 20mm\n    layerz 2\n}\n",
    )
    .expect("the board is written");

    let report = check(&broken);

    assert!(
        report.contains("has no property `layerz`"),
        "a typo in a design is still a typo:\n{report}"
    );
}

#[test]
fn a_kicad_board_under_its_own_name_is_unchanged() {
    let report = check(&fixture("led_blink.kicad_pcb"));

    assert!(
        !report.contains("cypcb::parse"),
        "the extension path still works:\n{report}"
    );
}
