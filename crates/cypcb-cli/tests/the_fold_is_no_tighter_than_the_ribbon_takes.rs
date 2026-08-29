//! A fold tighter than the ribbon takes.
//!
//! `cargo test -p cypcb-cli --test the_fold_is_no_tighter_than_the_ribbon_takes`
//!
//! The copper on the outside of a fold is stretched, and how far is set by the
//! radius against the ribbon's own thickness - which is why a house publishes
//! the limit as a multiple rather than as a length. JLCPCB states "Single
//! layer: >= 6x total thickness" and "Multi-layer: >= 10x total thickness".
//!
//! The thickness is the ribbon's, not the board's: a stiffener bonded under
//! the rigid end says `outside bend` and is not in the fold.

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
    let dir = std::env::temp_dir().join(format!("cypcb-folded-{who}"));
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

/// A ribbon of 0.110mm - coverlay, foil, core, foil - with a stiffener bonded
/// off the fold, and the fab and the radius as the variables.
fn board(fab: &str, radius: &str) -> String {
    format!(
        "version 1\n\n\
         board wearable {{\n    size 60mm x 16mm\n    layers 2\n    fab {fab}\n    stackup {{\n\
         \x20       coverlay 0.025mm material \"Kapton\" covers bend\n\
         \x20       copper 0.0175mm\n        core 0.05mm material \"Kapton\"\n\
         \x20       copper 0.0175mm\n\
         \x20       stiffener 0.2mm material \"FR4\" outside bend\n    }}\n}}\n\n\
         flex bend {{\n    bounds 22mm, 0mm to 38mm, 16mm\n    layer all\n{radius}\n}}\n"
    )
}

#[test]
fn a_fold_tighter_than_the_house_makes_is_reported_with_both_figures() {
    let said = check(&board("jlcpcb", "    radius 0.5mm"), "too-tight");
    assert!(said.contains("bend-radius"), "{said}");
    assert!(
        said.contains("the fold at 'bend' is 0.500mm and this house bends 2 copper layer(s) of 0.110mm no tighter than 10x that, which is 1.100mm"),
        "the fold, the ribbon, the multiple and the answer:\n{said}"
    );
}

#[test]
fn the_thickness_is_the_ribbons_and_not_the_boards() {
    // The stiffener is 0.2mm and says `outside bend`, so the fold is 0.110mm
    // rather than 0.310mm. At 10x those are 1.1mm and 3.1mm, and a radius of
    // 1.5mm is on opposite sides of them.
    let said = check(&board("jlcpcb", "    radius 1.5mm"), "ribbon-only");
    assert!(
        !said.contains("bend-radius"),
        "the stiffener is bonded off the fold and does not thicken it:\n{said}"
    );
    assert!(said.contains("passed DRC"), "{said}");
}

#[test]
fn a_fold_exactly_at_the_published_figure_is_a_fold_the_house_makes() {
    // 10 x 0.110mm is 1.100mm, and ">= 10x total thickness" includes it. A
    // rule that refused the boundary would turn the house's own figure into
    // one nobody can order.
    let said = check(&board("jlcpcb", "    radius 1.1mm"), "at-the-limit");
    assert!(
        !said.contains("bend-radius"),
        "the published figure is a radius, not a radius to beat:\n{said}"
    );
}

#[test]
fn a_region_that_states_no_radius_is_not_measured() {
    // Most designs say nothing, and nothing is invented for them.
    let said = check(&board("jlcpcb", ""), "unstated");
    assert!(
        !said.contains("bend-radius"),
        "no radius, nothing to measure:\n{said}"
    );
}

#[test]
fn a_house_that_published_no_figure_holds_the_design_to_nothing() {
    let said = check(&board("pcbway", "    radius 0.5mm"), "silent-house");
    assert!(
        !said.contains("bend-radius"),
        "a multiple invented here is one a designer gets turned away for:\n{said}"
    );
}
