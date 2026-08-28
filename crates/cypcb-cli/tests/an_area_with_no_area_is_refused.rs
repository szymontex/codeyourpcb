//! An area a design names has to be an area.
//!
//! `cargo test -p cypcb-cli --test an_area_with_no_area_is_refused`
//!
//! A design names an area so something else can point at it: `region
//! connector_end { bounds ... }`, then `stiffener 0.2mm covers connector_end`.
//! Every reader downstream takes the rectangle at its word - the handoff
//! document writes a stackup group for it, the 3D view asks whether a layer is
//! inside it, the copper filler cuts a pour to it.
//!
//! `bounds 10mm, 5mm to 10mm, 15mm` is four numbers like any other, and the
//! typo that produces it is one keystroke. Until this rule, nothing looked.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Check a board and hand back everything it printed.
///
/// The directory is named without the rule's word in it: every report carries
/// the board's path, so a temp directory called `cypcb-empty-area-...` would
/// make `contains("empty-area")` true for a board with no violation at all.
/// That is not hypothetical - it is how the first draft of the flex-hole test
/// passed while measuring nothing.
fn check(source: &str, who: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-collapsed-{who}"));
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

const BOARD: &str = r#"version 1

board panel {
    size 40mm x 20mm
    layers 2
}
"#;

#[test]
fn a_rectangle_whose_corners_share_an_edge_is_reported() {
    let said = check(
        &format!(
            "{BOARD}\nregion strip {{\n    bounds 10mm, 5mm to 10mm, 15mm\n    layer all\n}}\n"
        ),
        "no-width",
    );
    assert!(said.contains("empty-area"), "{said}");
    assert!(
        said.contains("the named area 'strip' has no width"),
        "the message names the area and which of the two collapsed:\n{said}"
    );
    assert!(
        said.contains("at (10.000mm, 5.000mm)"),
        "and sends the reader to the corner the design wrote first:\n{said}"
    );
}

#[test]
fn the_other_axis_reads_the_other_way_round() {
    let said = check(
        &format!(
            "{BOARD}\nkeepout under_can {{\n    bounds 4mm, 8mm to 30mm, 8mm\n    layer all\n}}\n"
        ),
        "no-height",
    );
    assert!(
        said.contains("the keepout 'under_can' has no height"),
        "each kind of area is called what the design called it:\n{said}"
    );
}

#[test]
fn an_area_that_is_an_area_is_not_reported() {
    // The half that keeps the rule from being noise: this is the shape every
    // board in `examples/` declares, and none of them may start failing.
    let said = check(
        &format!("{BOARD}\nregion connector_end {{\n    bounds 0mm, 0mm to 22mm, 16mm\n    layer all\n}}\n"),
        "ordinary",
    );
    assert!(
        !said.contains("empty-area"),
        "a rectangle with two dimensions is a rectangle:\n{said}"
    );
    assert!(said.contains("passed DRC"), "{said}");
}
