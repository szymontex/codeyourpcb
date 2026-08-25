//! A via too small for the fab's land is reported with both numbers.
//!
//! `cargo test -p cypcb-cli --test a_via_smaller_than_the_fab_lands`
//!
//! A via is a drill and the copper ring around it. The ring is what the
//! plating grabs and what a misregistered drill eats into, so every fab
//! publishes the smallest via land it will make - and where it publishes none,
//! this project derives one from the numbers it does publish - JLCPCB's floor
//! comes out at 0.50mm.
//!
//! `ViaDiameterRule` compares each via against that figure and nothing ran it.
//! It is the fourth of the six rules the registry census found with neither a
//! unit test nor a mention in any command-line test.
//!
//! A via's land here is twice its drill, which is what `sync` gives a via the
//! design does not size - so the way to ask for a small land is to ask for a
//! small hole.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A two-layer board with one via on the trace between two headers.
///
/// `{FAB}` names the house and `{DRILL}` the hole the via is drilled with.
const BOARD: &str = r#"version 1

board vias {
    size 30mm x 20mm
    layers 2
    fab {FAB}
}

component J1 connector "PIN-HDR-1x2" {
    value "in"
    at 5mm, 10mm
    rotate 90
}

component J2 connector "PIN-HDR-1x2" {
    value "out"
    at 25mm, 10mm
    rotate 90
}

net SIG {
    J1.1
    J2.1
}

trace SIG {
    from J1.1
    via 15mm, 10mm drill {DRILL}
    to J2.1
    layer Top
    width 0.2mm
}
"#;

fn check(who: &str, fab: &str, drill: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-via-land-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(
        &board,
        BOARD.replace("{FAB}", fab).replace("{DRILL}", drill),
    )
    .expect("the fixture is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&board)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn a_small_via_on_a_house_that_states_no_land_is_held_to_the_derived_one() {
    // 0.2mm drilled is a 0.40mm land, and JLCPCB publishes no via land, so the
    // floor is derived from the numbers it does publish: 0.50mm.
    let said = check("derived", "jlcpcb", "0.2mm");

    assert!(
        said.contains("via-diameter"),
        "a 0.40mm land is under what this house makes:\n{said}"
    );
    assert!(
        said.contains("Via diameter violation: 0.40mm actual, 0.50mm required"),
        "the report has to carry both numbers, because the fix is a choice \
         between a wider land and another fab:\n{said}"
    );
}

#[test]
fn the_same_board_drilled_to_the_houses_own_size_is_quiet() {
    // 0.3mm is JLCPCB's own minimum via drill, and twice that clears the land.
    let said = check("ok", "jlcpcb", "0.3mm");
    assert!(
        !said.contains("via-diameter"),
        "this via is the size the house asks for:\n{said}"
    );
}

#[test]
fn another_table_puts_the_floor_somewhere_else() {
    // The floor is the house's, not this project's: the same via that passes
    // at JLCPCB fails on the `prototype` table, whose land is 0.8mm.
    let said = check("published", "prototype", "0.3mm");
    assert!(
        said.contains("Via diameter violation: 0.60mm actual, 0.80mm required"),
        "a via is graded against the table the board is checked with:\n{said}"
    );
}

#[test]
fn the_shipped_blind_via_example_stays_quiet() {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", "examples/blind-via.cypcb"])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        !said.contains("via-diameter"),
        "the example's vias are the size its house makes:\n{said}"
    );
}
