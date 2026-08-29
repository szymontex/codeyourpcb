//! A stiffener a house does not bond.
//!
//! `cargo test -p cypcb-cli --test the_stiffener_is_a_sheet_the_fab_stocks`
//!
//! A stiffener is a sheet of FR4, polyimide or steel laminated under the rigid
//! part of a flex board, and a fabricator bonds the sheets it stocks rather
//! than any figure a design asks for. JLCPCB publishes three lists of
//! thickness options on its flex capabilities page - PI at 0.1, 0.15, 0.20,
//! 0.225 and 0.25mm, FR4 at 0.1, 0.2, 0.4, 0.6, 0.8, 1.0, 1.2 and 1.6mm,
//! stainless steel at 0.1, 0.2 and 0.3mm - and 0.25mm is on one of those lists
//! and not on the other two, which is the whole point of asking per material.
//!
//! A table that has read no list says nothing. A design held to a figure
//! nobody published is worse than a design held to nothing.

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
    let dir = std::env::temp_dir().join(format!("cypcb-bonded-sheet-{who}"));
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

/// A flex build whose fab and stiffener are the variables.
fn board(fab: &str, stiffener: &str) -> String {
    format!(
        "version 1\n\n\
         board panel {{\n    size 60mm x 16mm\n    layers 2\n    fab {fab}\n    stackup {{\n\
         \x20       copper 0.5oz\n        core 0.05mm material \"Kapton\"\n        copper 0.5oz\n\
         \x20       {stiffener}\n    }}\n}}\n"
    )
}

#[test]
fn a_thickness_the_house_does_not_stock_is_reported_with_the_ones_it_does() {
    let said = check(
        &board("jlcpcb", "stiffener 0.25mm material \"FR4\""),
        "quarter-fr4",
    );
    assert!(
        said.contains("asks for a 0.250mm FR4 stiffener and this house bonds"),
        "the figure asked for, and whose it is:\n{said}"
    );
    assert!(
        said.contains("0.100mm, 0.200mm, 0.400mm, 0.600mm, 0.800mm, 1.000mm, 1.200mm, 1.600mm"),
        "and every sheet the house publishes, so the designer can pick one:\n{said}"
    );
}

#[test]
fn the_same_figure_on_the_material_that_stocks_it_passes() {
    // 0.25mm is on the polyimide list and not on the FR4 one. A rule that
    // measured thickness without the material would answer the same for both.
    let said = check(
        &board("jlcpcb", "stiffener 0.25mm material \"PI\""),
        "quarter-pi",
    );
    assert!(
        !said.contains("stiffener and this house bonds"),
        "polyimide is stocked at 0.25mm:\n{said}"
    );
}

#[test]
fn a_sheet_that_is_stocked_is_not_reported() {
    let said = check(
        &board("jlcpcb", "stiffener 0.2mm material \"FR4\""),
        "stocked",
    );
    assert!(
        !said.contains("stiffener and this house bonds"),
        "0.2mm FR4 is on the published list:\n{said}"
    );
}

#[test]
fn a_house_that_published_no_list_holds_the_design_to_nothing() {
    // The half that keeps this honest: a figure invented for a house that
    // publishes nothing is a figure a designer gets turned away for.
    let said = check(
        &board("pcbway", "stiffener 0.25mm material \"FR4\""),
        "silent-house",
    );
    assert!(
        !said.contains("stiffener and this house bonds"),
        "no published list, nothing to hold the design to:\n{said}"
    );
}

#[test]
fn a_material_the_house_publishes_no_list_for_is_left_alone() {
    let said = check(
        &board("jlcpcb", "stiffener 0.25mm material \"Aluminium\""),
        "other-material",
    );
    assert!(
        !said.contains("stiffener and this house bonds"),
        "the design may well be right, and this rule cannot measure it:\n{said}"
    );
}
