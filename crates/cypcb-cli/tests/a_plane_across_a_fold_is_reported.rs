//! A plane poured solid across a fold.
//!
//! `cargo test -p cypcb-cli --test a_plane_across_a_fold_is_reported`
//!
//! IPC-2223 asks for a hatched polygon in the flex area rather than solid
//! copper: a sheet of copper over a fold takes the strain across an unbroken
//! surface and cracks where the fold begins, which is the same failure as a
//! trace running along a bend at the width of the whole plane.
//!
//! This tool fills a pour solid, so the checker reports the overlap rather
//! than quietly hatching a plane the design asked for - and it measures it,
//! because a designer who reads `4.000mm by 16.000mm` knows how much of the
//! plane is in the fold.

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
    let dir = std::env::temp_dir().join(format!("cypcb-sheet-{who}"));
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

/// A ribbon at 22..38mm across a 60mm board, with the pour as the variable.
fn board(pour: &str) -> String {
    format!(
        "version 1\n\n\
         board ribbon {{\n    size 60mm x 16mm\n    layers 2\n}}\n\n\
         net GND {{\n}}\n\n\
         {pour}\n\n\
         flex bend {{\n    bounds 22mm, 0mm to 38mm, 16mm\n    layer all\n}}\n"
    )
}

const REACHING_IN: &str =
    "zone GND {\n    bounds 18mm, 0mm to 26mm, 16mm\n    layer top\n    net GND\n}";

#[test]
fn the_copper_in_the_fold_is_measured() {
    let said = check(&board(REACHING_IN), "reaching-in");
    assert!(said.contains("solid-pour-in-bend"), "{said}");
    assert!(
        said.contains(
            "the pour 'GND' covers 4.000mm by 16.000mm of 'bend' from (22.000mm, 0.000mm)"
        ),
        "how much of the plane is in the fold, and where:\n{said}"
    );
}

#[test]
fn a_plane_that_stops_at_the_fold_is_the_design_being_right() {
    // The same plane, ending where the ribbon begins. This is what a designer
    // does about the fault, so the rule has to be silent about it.
    let said = check(
        &board("zone GND {\n    bounds 4mm, 0mm to 22mm, 16mm\n    layer top\n    net GND\n}"),
        "stops-short",
    );
    assert!(
        !said.contains("solid-pour-in-bend"),
        "an edge is not an overlap:\n{said}"
    );
}

#[test]
fn a_plane_on_a_layer_the_ribbon_does_not_reach_is_left_alone() {
    // A region stated on the top layer only, and a plane on the bottom: the
    // two are not over each other in any sense a fabricator cares about.
    let source = board(REACHING_IN)
        .replace(
            "    layer top\n    net GND",
            "    layer bottom\n    net GND",
        )
        .replace(
            "flex bend {\n    bounds 22mm, 0mm to 38mm, 16mm\n    layer all\n}",
            "flex bend {\n    bounds 22mm, 0mm to 38mm, 16mm\n    layer top\n}",
        );
    let said = check(&source, "other-layer");
    assert!(
        !said.contains("solid-pour-in-bend"),
        "the plane and the fold share no copper:\n{said}"
    );
}

#[test]
fn a_board_that_does_not_bend_hears_nothing() {
    let source = board(REACHING_IN).replace(
        "flex bend {\n    bounds 22mm, 0mm to 38mm, 16mm\n    layer all\n}",
        "",
    );
    let said = check(&source, "rigid");
    assert!(
        !said.contains("solid-pour-in-bend"),
        "a rigid board has no fold to pour across:\n{said}"
    );
}
