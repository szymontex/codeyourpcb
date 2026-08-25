//! Nothing is drilled where the board bends.
//!
//! `cargo test -p cypcb-cli --test a_hole_in_the_bend_is_refused`
//!
//! The barrel of a plated hole is a tube of copper on the wall of the hole.
//! The laminate around it moves every time the board is folded and the barrel
//! does not, so it work-hardens and splits - usually at the knee where the
//! plating meets the pad, and usually after the product has shipped. Every
//! flex design guide says the same thing in the same words: no holes in the
//! bend.
//!
//! `FlexHoleRule` says it about a specific hole, at its coordinates, and
//! nothing ran it. It was one of six rules with neither a unit test nor a
//! mention in any command-line test, found by counting the rule registry
//! against the suite - and it is the rule behind V8's rigid-flex vocabulary,
//! which is the part of the language the owner asked for last.
//!
//! `examples/rigid-flex.cypcb` is the board that keeps the other half honest:
//! its flex region is crossed by copper and nothing is drilled in it, and it
//! passes.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A board with a bend across the middle, and a footprint whose pads are
/// drilled.
///
/// `{AT}` is where the drilled part is placed: inside the bend or clear of it.
const BOARD: &str = r#"version 1

board ribbon {
    size 40mm x 20mm
    layers 2
}

flex bend {
    bounds 15mm, 0mm to 25mm, 20mm
    layer all
}

footprint THT2 {
    description "two drilled pads"
    courtyard 4mm x 3mm
    pad 1 rect at -1.27mm, 0mm size 1.6mm x 1.6mm drill 0.8mm
    pad 2 rect at 1.27mm, 0mm size 1.6mm x 1.6mm drill 0.8mm
}

component J1 connector "THT2" {
    value "tail"
    at {AT}
}
"#;

fn check(who: &str, at: &str) -> String {
    // The directory is named without the rule's own word in it: the first
    // draft called it `cypcb-flex-hole-...` and every report carries the
    // board's path, so the test that asserts the rule stays quiet read its
    // own temp directory as a violation.
    let dir = std::env::temp_dir().join(format!("cypcb-bend-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, BOARD.replace("{AT}", at)).expect("the fixture is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&board)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn a_drilled_pad_inside_the_bend_is_reported_with_its_size() {
    let said = check("inside", "20mm, 10mm");

    assert!(
        said.contains("flex-hole"),
        "a hole in the bend is the one thing every flex guide refuses:\n{said}"
    );
    assert!(
        said.contains("sits in the flexible region 'bend'"),
        "and the report has to name the region the design named:\n{said}"
    );
    assert!(
        said.contains("0.800mm across"),
        "and the hole, so a reader knows which one:\n{said}"
    );
}

#[test]
fn the_same_part_clear_of_the_bend_is_not() {
    // The half that keeps the other from being noise: the same board with the
    // part on the rigid end.
    let said = check("clear", "6mm, 10mm");
    assert!(
        !said.contains("flex-hole"),
        "this hole is nowhere near the bend:\n{said}"
    );
}

#[test]
fn the_shipped_flex_example_has_nothing_drilled_in_its_bend() {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", "examples/rigid-flex.cypcb"])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        !said.contains("flex-hole"),
        "the example's own header says nothing is drilled in the bend:\n{said}"
    );
}
