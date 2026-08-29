//! A mesh the fabricator cannot etch.
//!
//! `cargo test -p cypcb-cli --test the_mesh_is_one_the_house_can_etch`
//!
//! `hatch 0.3mm pitch 1mm` is two figures the filler turns into copper. Both
//! are held to the numbers every other feature on the board is held to: a line
//! of a mesh is a trace as far as the etch bath is concerned, and the space
//! between two lines is clearance. Until this rule nothing compared them, so
//! `hatch 0.05mm pitch 0.06mm` went to the fab and came back as a question.
//!
//! The gap is derived rather than stated - pitch less width - which is why the
//! rule reads both figures rather than only the one the design wrote about
//! copper.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn check(source: &str, who: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-etch-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, source).expect("the board is written");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", board.to_str().expect("a path that is text")])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

/// A plain board with one hatched pour, the mesh as the variable.
fn board(hatch: &str) -> String {
    format!(
        "version 1\n\n\
         board panel {{\n    size 20mm x 20mm\n    layers 2\n    fab jlcpcb\n}}\n\n\
         net GND {{\n}}\n\n\
         zone GND {{\n    bounds 2mm, 2mm to 12mm, 12mm\n    layer top\n    net GND\n    {hatch}\n}}\n"
    )
}

#[test]
fn a_line_thinner_than_the_house_etches_is_reported_with_both_figures() {
    let said = check(&board("hatch 0.05mm pitch 1mm"), "thin-lines");
    assert!(
        said.contains("hatched with 0.050mm lines and this house etches 0.127mm"),
        "the line the design asked for, and the one the table holds:\n{said}"
    );
}

#[test]
fn the_gap_between_lines_is_derived_and_held_to_the_clearance() {
    // 0.2mm lines a quarter of a millimetre apart leaves 0.05mm of laminate,
    // and JLCPCB holds 0.127mm. The design never wrote 0.05mm anywhere: it is
    // the pitch less the width, which is the half a designer forgets.
    let said = check(&board("hatch 0.2mm pitch 0.25mm"), "narrow-gap");
    assert!(
        said.contains("leaves 0.050mm between its lines - 0.250mm pitch less 0.200mm of copper - and this house holds 0.127mm"),
        "the arithmetic spelled out, so the figure to change is obvious:\n{said}"
    );
}

#[test]
fn lines_that_touch_are_a_sheet_and_the_design_is_told_so() {
    // The filler leaves such a pour solid rather than cutting a mesh with no
    // gaps in it. A designer who wrote this meant to hatch something.
    let said = check(&board("hatch 0.5mm pitch 0.5mm"), "touching");
    assert!(
        said.contains("states a 0.500mm pitch and 0.500mm lines, so its lines touch"),
        "{said}"
    );
}

#[test]
fn a_mesh_the_house_makes_is_not_reported() {
    // The half that keeps the rule from being noise, and the mesh
    // `examples/rigid-flex.cypcb` states: 0.3mm lines, 0.7mm of gap, both well
    // inside JLCPCB's 0.127mm.
    let said = check(&board("hatch 0.3mm pitch 1mm"), "ordinary");
    assert!(
        !said.contains("hatch-too-fine"),
        "0.3mm of copper and 0.7mm of gap is a mesh any house etches:\n{said}"
    );
    // Not `passed DRC`: this fixture has no parts on it, so its plane reaches
    // no pad of its own net and `pour-island` says so. That is a different
    // rule answering a different question about a board written to ask this
    // one.
}

#[test]
fn a_pour_that_asked_for_no_mesh_is_not_measured() {
    let source = board("hatch 0.3mm pitch 1mm").replace("    hatch 0.3mm pitch 1mm\n", "");
    let said = check(&source, "solid");
    assert!(
        !said.contains("hatch-too-fine"),
        "a solid pour states no mesh to measure:\n{said}"
    );
}
