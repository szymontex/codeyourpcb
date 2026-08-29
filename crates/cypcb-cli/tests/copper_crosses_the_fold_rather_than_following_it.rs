//! Copper crossing a fold has to cross it, not follow it.
//!
//! `cargo test -p cypcb-cli --test copper_crosses_the_fold_rather_than_following_it`
//!
//! IPC-2223 routes a bend area perpendicular to the bend, so every conductor
//! takes the same strain over the fold. A trace running along the fold takes
//! that strain along its own length, concentrated where it enters and leaves
//! the bend, and cracks there.
//!
//! The fold direction is read from the region: a ribbon reaching the top and
//! bottom edges folds about a line running that way, so copper crosses it left
//! to right. A patch in the middle of a board says nothing about which way it
//! folds, and this rule says nothing about it.

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
    let dir = std::env::temp_dir().join(format!("cypcb-strain-{who}"));
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

/// A ribbon across a 60mm by 16mm board, with the copper and the region as the
/// variables. The band reaches the top and bottom edges, so copper crosses it
/// left to right.
fn board(path: &str, region: &str) -> String {
    format!(
        "version 1\n\n\
         board ribbon {{\n    size 60mm x 16mm\n    layers 2\n}}\n\n\
         net SIG {{\n}}\n\n\
         trace SIG {{\n    layer top\n    width 0.2mm\n    path {path}\n}}\n\n\
         {region}\n"
    )
}

const RIBBON: &str = "flex bend {\n    bounds 22mm, 0mm to 38mm, 16mm\n    layer all\n}";

#[test]
fn copper_along_the_fold_is_reported_with_the_angle_it_is_off_by() {
    let said = check(&board("30mm, 2mm -> 30mm, 14mm", RIBBON), "along");
    assert!(said.contains("flex-trace-angle"), "{said}");
    assert!(
        said.contains("runs 90 degrees off the fold's own direction"),
        "the measured angle, not a word for it:\n{said}"
    );
}

#[test]
fn copper_across_the_fold_is_what_the_rule_asks_for() {
    let said = check(&board("24mm, 8mm -> 36mm, 8mm", RIBBON), "across");
    assert!(
        !said.contains("flex-trace-angle"),
        "a trace perpendicular to the fold is the design being right:\n{said}"
    );
}

#[test]
fn a_lean_short_of_forty_five_degrees_is_still_crossing_it() {
    // 12mm across and 6mm along is about 27 degrees off - not perpendicular,
    // and still crossing the fold rather than following it. A rule that
    // reported it would fire on most real ribbons, where copper fans out to
    // reach its pads.
    let said = check(&board("24mm, 5mm -> 36mm, 11mm", RIBBON), "leaning");
    assert!(
        !said.contains("flex-trace-angle"),
        "27 degrees off is the design being nearly right:\n{said}"
    );

    // Past 45 the same trace is more along the fold than across it: 6mm across
    // and 12mm along is about 63 degrees.
    let said = check(&board("28mm, 2mm -> 34mm, 14mm", RIBBON), "leaning-far");
    assert!(
        said.contains("runs 63 degrees off"),
        "and past the halfway point it is the design being wrong:\n{said}"
    );
}

#[test]
fn copper_outside_the_bend_is_not_measured() {
    // The same vertical trace, on the rigid end. Nothing bends there.
    let said = check(&board("10mm, 2mm -> 10mm, 14mm", RIBBON), "rigid-end");
    assert!(
        !said.contains("flex-trace-angle"),
        "a rigid part of the board is not a fold:\n{said}"
    );
}

#[test]
fn a_region_that_does_not_span_the_board_says_nothing_about_its_fold() {
    // A patch in the middle: it reaches neither pair of edges, so which way it
    // folds is not something this rule can read out of the shape.
    let patch = "flex bend {\n    bounds 22mm, 4mm to 38mm, 12mm\n    layer all\n}";
    let said = check(&board("30mm, 5mm -> 30mm, 11mm", patch), "patch");
    assert!(
        !said.contains("flex-trace-angle"),
        "no fold direction to measure against:\n{said}"
    );
}
