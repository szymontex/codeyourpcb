//! A hole can be too deep for its own width to be plated.
//!
//! `cargo test -p cypcb-cli --test a_hole_too_deep_to_plate`
//!
//! Plating a through hole is chemistry rather than machining: copper is pulled
//! down the barrel out of solution, and past some depth-to-width ratio the
//! solution stops refreshing in the middle. The board comes back with a barrel
//! that is thin or open somewhere nobody can see. Every fab publishes the
//! ratio it will still plate - 8:1 on JLCPCB's standard process - and
//! `DrillAspectRatioRule` divides the board's thickness by each drill.
//!
//! Nothing ran it. It is the third of six rules the registry census found with
//! neither a unit test nor a mention in any command-line test, and the one
//! whose arithmetic this project has got wrong twice: the depth is the
//! **design's own stackup** when it states one, and the fab's standard
//! thickness when it does not.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A two-layer board with one drilled part on it.
///
/// `{STACK}` is the stackup, if the case states one, and `{PAD}` is the pad
/// line - which decides how wide the hole is and whether it is plated.
const BOARD: &str = r#"version 1

board holes {
    size 20mm x 20mm
    layers 2
    fab jlcpcb
{STACK}}

footprint ONE {
    description "one hole"
    courtyard 3mm x 3mm
    {PAD}
}

component J1 connector "ONE" {
    value "hole"
    at 10mm, 10mm
}
"#;

fn check(who: &str, stack: &str, pad: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-deep-hole-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(
        &board,
        BOARD.replace("{STACK}", stack).replace("{PAD}", pad),
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

/// A plated hole, at the width the case asks for.
fn plated(drill: &str) -> String {
    format!("pad 1 circle at 0mm, 0mm size 1.2mm x 1.2mm drill {drill}")
}

/// A thin flexible build: 0.4mm from face to face, so the same drill is a
/// quarter of the depth it would be on a 1.6mm board.
const THIN: &str = r#"
    stackup {
        copper 0.5oz
        core 0.35mm material "Kapton" dk 3.4
        copper 0.5oz
    }
"#;

#[test]
fn a_narrow_hole_through_a_standard_board_is_reported_with_both_numbers() {
    // No stackup, so the board is JLCPCB's standard 1.6mm, and 8:1 puts the
    // smallest platable drill at 0.2mm.
    let said = check("standard", "", &plated("0.15mm"));

    assert!(
        said.contains("drill-aspect-ratio"),
        "0.15mm through 1.6mm is more than 8:1:\n{said}"
    );
    assert!(
        said.contains("A 0.150mm hole through a 1.60mm board is 10.7:1"),
        "the report has to state the hole, the board and the ratio, because \
         the fix is a choice between them:\n{said}"
    );
    assert!(
        said.contains("0.200mm is the smallest that reaches"),
        "and the width that would work:\n{said}"
    );
}

#[test]
fn the_same_hole_through_the_boards_own_thin_stack_is_fine() {
    // The half the arithmetic has been wrong about twice: a design that states
    // how it is built states how deep its holes are. 0.4mm at 8:1 reaches
    // 0.05mm, so 0.15mm is no longer deep.
    let said = check("thin", THIN, &plated("0.15mm"));
    assert!(
        !said.contains("drill-aspect-ratio"),
        "this board is 0.4mm thick, not 1.6mm:\n{said}"
    );
}

#[test]
fn the_same_hole_on_a_house_that_plates_deeper_is_fine() {
    // The other number in the division: JLCPCB's advanced four-layer process
    // plates 12:1 where its standard one plates 8:1, so the same 0.15mm hole
    // through the same 1.6mm board reaches on one and not the other.
    let dir = std::env::temp_dir().join("cypcb-deep-hole-advanced");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(
        &board,
        BOARD
            .replace("{STACK}", "")
            .replace("{PAD}", &plated("0.15mm"))
            .replace("layers 2", "layers 4")
            .replace("fab jlcpcb", "fab jlcpcb_advanced_4layer"),
    )
    .expect("the fixture is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&board)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        !said.contains("drill-aspect-ratio"),
        "12:1 reaches 0.133mm, and this hole is 0.15mm:\n{said}"
    );
}

#[test]
fn a_hole_exactly_at_the_limit_reaches() {
    // 1.6mm at 8:1 is 0.2mm exactly, and the fab publishes the ratio it will
    // still plate rather than the first one it will not. A rule that reported
    // the boundary would send a designer looking for a wider drill than the
    // house asks for.
    let said = check("limit", "", &plated("0.2mm"));
    assert!(
        !said.contains("drill-aspect-ratio"),
        "0.2mm through 1.6mm is 8:1, which this fab plates:\n{said}"
    );
}

#[test]
fn the_shipped_slotted_connector_stays_quiet() {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", "examples/slotted-connector.cypcb"])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        !said.contains("drill-aspect-ratio"),
        "every hole on that board is wide enough for its depth:\n{said}"
    );
}
