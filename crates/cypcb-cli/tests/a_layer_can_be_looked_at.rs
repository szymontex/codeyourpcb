//! `--svg` plots a copper layer for a person.
//!
//! `cargo test -p cypcb-cli --test a_layer_can_be_looked_at`
//!
//! Gerber is what a fabricator reads. Until this, nothing in the tool drew a
//! layer anybody could look at without a Gerber viewer - no picture for a
//! review, none for a document, none for a web page. Item 7 of the KiCad
//! parity audit.
//!
//! SVG is text, so what was drawn can be read back: the size of the page, the
//! one flip that turns board coordinates into screen ones, a line per track at
//! its own width, and a shape per pad.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
        .join(name)
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-svg-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Export with plots and read one back.
fn plot(board: &Path, out: &Path, suffix: &str) -> String {
    let status = cypcb()
        .arg("export")
        .arg(board)
        .arg("-o")
        .arg(out)
        .arg("--svg")
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");

    let dir = out.join("plot");
    let file = std::fs::read_dir(&dir)
        .expect("the plot directory exists")
        .map(|entry| entry.expect("a directory entry").path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(&format!("{suffix}.svg")))
        })
        .unwrap_or_else(|| panic!("a {suffix} plot was written"));
    std::fs::read_to_string(file).expect("the plot is readable")
}

#[test]
fn the_page_is_the_board_at_size() {
    // usb-diff-pair is 30mm by 20mm, and a viewer printing at 100% has to
    // print it at that: a plot that fits the page is a plot nobody can measure.
    let svg = plot(&example("usb-diff-pair.cypcb"), &scratch("size"), "F_Cu");
    assert!(
        svg.contains("width=\"30.000mm\" height=\"20.000mm\""),
        "the page is the board's own size:\n{}",
        &svg[..svg.len().min(400)]
    );
    assert!(
        svg.contains("viewBox=\"0 0 30.000 20.000\""),
        "one user unit is one millimetre"
    );
}

#[test]
fn the_axis_is_flipped_once_for_the_whole_drawing() {
    // A board's Y grows up and an SVG's grows down. Doing that per shape is
    // six chances to be wrong; this is one.
    let svg = plot(&example("usb-diff-pair.cypcb"), &scratch("flip"), "F_Cu");
    assert!(
        svg.contains("<g transform=\"translate(0 20.000) scale(1 -1)\">"),
        "the drawing is inside one flipped group:\n{svg}"
    );
    assert!(
        svg.contains("y1=\"6.730\""),
        "and the shapes inside it are in the board's own coordinates:\n{svg}"
    );
}

#[test]
fn every_track_is_a_line_of_its_own_width() {
    let svg = plot(&example("usb-diff-pair.cypcb"), &scratch("tracks"), "F_Cu");
    let lines: Vec<&str> = svg.lines().filter(|line| line.contains("<line")).collect();
    assert_eq!(lines.len(), 2, "the board has two tracks:\n{svg}");
    for line in &lines {
        assert!(
            line.contains("stroke-width=\"0.200\""),
            "a track is drawn at the width it is: {line}"
        );
        assert!(
            line.contains("stroke-linecap=\"round\""),
            "and it ends the way copper ends: {line}"
        );
    }
}

#[test]
fn each_copper_layer_gets_its_own_picture() {
    let out = scratch("layers");
    let top = plot(&example("usb-diff-pair.cypcb"), &out, "F_Cu");
    let bottom = std::fs::read_to_string(out.join("plot").join("usb-diff-pair-B_Cu.svg"))
        .expect("the bottom plot is readable");

    // Through-hole pads are on both layers; the tracks are only on the top.
    assert!(top.contains("<line"), "the top carries the tracks");
    assert!(
        !bottom.contains("<line"),
        "the bottom carries none, because none are routed there:\n{bottom}"
    );
    assert!(
        bottom.contains("<circle") || bottom.contains("<rect"),
        "but it does carry the pads that go through the board"
    );
}

#[test]
fn a_board_that_does_not_ask_gets_no_plots() {
    let out = scratch("silent");
    let status = cypcb()
        .arg("export")
        .arg(example("usb-diff-pair.cypcb"))
        .arg("-o")
        .arg(&out)
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    assert!(
        !out.join("plot").exists(),
        "the file set a house receives is unchanged unless a plot is asked for"
    );
}
