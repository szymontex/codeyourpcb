//! A pad shape this importer has no word for is named, not swallowed.
//!
//! `cargo test -p cypcb-cli --test a_pad_shape_the_importer_cannot_carry_is_named`
//!
//! The board reader answers an unknown pad shape with a rectangle, and that is
//! the right answer for a board: one odd pad should not cost a person the
//! other nine hundred. It was doing it in silence, which is a different thing.
//! `custom` is a polygon somebody drew and `trapezoid` is not a rectangle, so
//! the substitution changes copper - and the checker, the router and the
//! Gerber writer all take the substitute for the shape in the file.
//!
//! The footprint reader already refuses an unknown shape by name. The board
//! reader now says what it approximated, the way it already says which pours
//! it would not carry.

use std::path::PathBuf;
use std::process::Command;

/// A board with one pad KiCad wrote as `custom`.
const CUSTOM: &str = include_str!("fixtures/a_custom_pad.kicad_pcb");

/// Run `check` on a board written into a temp directory, and hand back stderr.
fn warnings(who: &str, board: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-shape-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let file: PathBuf = dir.join("board.kicad_pcb");
    std::fs::write(&file, board).expect("the board is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&file)
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn a_custom_pad_says_what_it_became() {
    let said = warnings("custom", CUSTOM);

    assert!(
        said.contains("states shape `custom`"),
        "the shape the file states has to appear: {said}"
    );
    assert!(said.contains("pad 1"), "and which pad it was: {said}");
    assert!(
        said.contains("rectangle"),
        "and what it became, because that is the part that changes copper: {said}"
    );
}

#[test]
fn a_board_of_shapes_this_reader_knows_says_nothing() {
    // The control. A warning on every board is a warning nobody reads, and an
    // absence proves nothing unless the same command can produce one.
    let known = CUSTOM.replace(" custom ", " rect ");
    assert_ne!(known, CUSTOM, "the control has to differ from the case");

    let said = warnings("known", &known);
    assert!(
        !said.contains("states shape"),
        "a board of rect, circle, oval and roundrect pads has nothing to say: {said}"
    );
}
