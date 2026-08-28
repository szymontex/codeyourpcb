//! A declared area has to be on the board.
//!
//! `cargo test -p cypcb-cli --test an_area_off_the_board_is_refused`
//!
//! `empty-area` asks whether a declared area is an area; this asks where it
//! is. `region connector_end { bounds 0mm, 0mm to 22mm, 16mm }` is inside a
//! 40mm board and hangs 2mm off a 20mm one, and a `covers connector_end`
//! clause pointing at the second orders a stiffener over air.
//!
//! The overhang is measured rather than described, because a designer who
//! reads `hangs 2.000mm off the right edge` knows which number to change.

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
/// The directory carries none of the rule's words: every report prints the
/// board's path, so a directory named after the rule would make
/// `contains("area-off-board")` true for a board with nothing wrong with it.
fn check(source: &str, who: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-overhang-{who}"));
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

/// The area that made this rule worth writing, with the board size left open.
fn board_of(width_mm: u32, area: &str) -> String {
    format!(
        "version 1\n\nboard panel {{\n    size {width_mm}mm x 20mm\n    layers 2\n}}\n\n{area}\n"
    )
}

const CONNECTOR_END: &str =
    "region connector_end {\n    bounds 0mm, 0mm to 22mm, 16mm\n    layer all\n}";

#[test]
fn the_overhang_is_measured_and_the_edge_is_named() {
    let said = check(&board_of(20, CONNECTOR_END), "hangs-off");
    assert!(
        said.contains("hangs 2.000mm off the right edge of a 20.000mm by 20.000mm board"),
        "the number a designer has to change, and the side it is on:\n{said}"
    );
    assert!(said.contains("area-off-board"), "{said}");
}

#[test]
fn an_area_nowhere_near_the_board_reads_differently() {
    // Hanging off an edge and being in another coordinate system are the same
    // arithmetic and different mistakes.
    let said = check(
        &board_of(
            20,
            "keepout far_away {\n    bounds 40mm, 40mm to 50mm, 50mm\n    layer all\n}",
        ),
        "elsewhere",
    );
    assert!(
        said.contains("the keepout 'far_away' sits entirely off a 20.000mm by 20.000mm board"),
        "{said}"
    );
    assert!(
        !said.contains("hangs"),
        "an area with no overhang to name does not name one:\n{said}"
    );
}

#[test]
fn an_area_over_two_edges_names_the_one_it_hangs_furthest_off() {
    // 25mm of area on a 20mm board is 10mm over the right edge and 2mm over
    // the top. Naming the smaller of the two sends the designer to the number
    // that is nearly right.
    let said = check(
        &board_of(
            20,
            "region corner {\n    bounds 5mm, 5mm to 30mm, 22mm\n    layer all\n}",
        ),
        "two-edges",
    );
    assert!(
        said.contains("hangs 10.000mm off the right edge"),
        "the larger overhang is the one worth naming:\n{said}"
    );
    assert!(
        !said.contains("top edge"),
        "and the smaller one is not what the message says:\n{said}"
    );
}

#[test]
fn the_same_area_on_a_board_that_fits_it_is_not_reported() {
    // The half that keeps the rule from being noise: one number wider and the
    // same design is right.
    let said = check(&board_of(40, CONNECTOR_END), "fits");
    assert!(
        !said.contains("area-off-board"),
        "22mm of area on a 40mm board is on the board:\n{said}"
    );
    assert!(said.contains("passed DRC"), "{said}");
}

#[test]
fn a_pour_off_the_board_is_left_to_the_rule_that_already_measures_copper() {
    // `edge-clearance` holds copper to the fab's distance from the edge and
    // reports a plane hanging off the board. Two rows for one fault teach a
    // reader to skim the panel, so this rule says nothing about a pour.
    let said = check(
        &board_of(
            20,
            "net GND {\n}\n\nzone GND {\n    bounds 0mm, 0mm to 22mm, 16mm\n    layer top\n    net GND\n}",
        ),
        "pour",
    );
    assert!(
        !said.contains("area-off-board"),
        "a pour is copper, and copper against the edge is edge-clearance's question:\n{said}"
    );
    assert!(
        said.contains("edge-clearance"),
        "and it is still reported, by that rule:\n{said}"
    );
}
