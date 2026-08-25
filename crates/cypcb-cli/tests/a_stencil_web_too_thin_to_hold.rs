//! The stencil has to survive being made.
//!
//! `cargo test -p cypcb-cli --test a_stencil_web_too_thin_to_hold`
//!
//! A solder paste stencil is a steel sheet with a hole cut for every SMD pad.
//! Where two holes come closer than the fab's paste clearance, the web of
//! steel between them is too thin to hold: it tears, the two openings become
//! one, and the parts bridge with solder on reflow.
//!
//! `PasteClearanceRule` is the last of the six rules the registry census found
//! with neither a unit test nor a mention in any command-line test. Every fab
//! preset has published a paste clearance since the tables were written -
//! JLCPCB's standard process is 0.127mm - and until the rule was written
//! nothing read the number; until now nothing ran the rule.
//!
//! A through-hole pad gets no aperture at all: it is soldered by wave or by
//! hand, so there is no steel between anything to tear.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A board carrying one two-pad part.
///
/// `{PADS}` is the pair of pad lines, which decides how wide the web between
/// their apertures is and whether they are stencilled at all.
const BOARD: &str = r#"version 1

board stencil {
    size 20mm x 20mm
    layers 2
    fab jlcpcb
}

footprint PAIR {
    description "two pads, close together"
    courtyard 3mm x 2mm
{PADS}}

component U1 ic "PAIR" {
    value "fine pitch"
    at 10mm, 10mm
}
"#;

fn check(who: &str, pads: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-stencil-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, BOARD.replace("{PADS}", pads)).expect("the fixture is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&board)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

/// Two surface pads whose edges are `gap` apart, written as their positions.
const TIGHT: &str = r#"    pad 1 rect at -0.3mm, 0mm size 0.5mm x 0.5mm
    pad 2 rect at 0.3mm, 0mm size 0.5mm x 0.5mm
"#;

const CLEAR: &str = r#"    pad 1 rect at -0.5mm, 0mm size 0.5mm x 0.5mm
    pad 2 rect at 0.5mm, 0mm size 0.5mm x 0.5mm
"#;

/// The same tight pair, drilled - so soldered by wave or by hand.
const DRILLED: &str = r#"    pad 1 rect at -0.3mm, 0mm size 0.5mm x 0.5mm drill 0.3mm
    pad 2 rect at 0.3mm, 0mm size 0.5mm x 0.5mm drill 0.3mm
"#;

#[test]
fn two_apertures_with_a_thin_web_between_them_are_reported() {
    // 0.6mm apart, 0.5mm wide: 0.1mm of steel, where this fab asks for
    // 0.127mm.
    let said = check("tight", TIGHT);

    assert!(
        said.contains("paste-clearance"),
        "0.1mm of steel is thinner than this fab will cut:\n{said}"
    );
    assert!(
        said.contains("Paste stencil web is 0.100mm, 0.127mm required"),
        "the report has to carry both numbers, because the fix is a pad \
         change or a fab change:\n{said}"
    );
}

#[test]
fn the_same_pads_further_apart_are_quiet() {
    // 0.5mm of steel between them.
    let said = check("clear", CLEAR);
    assert!(
        !said.contains("paste-clearance"),
        "half a millimetre of steel holds:\n{said}"
    );
}

#[test]
fn a_through_hole_pair_gets_no_aperture_to_tear() {
    // The same tight geometry, drilled. There is no stencil opening over a
    // through-hole pad, so there is no web between anything.
    let said = check("drilled", DRILLED);
    assert!(
        !said.contains("paste-clearance"),
        "these pads are soldered by wave or by hand:\n{said}"
    );
}

#[test]
fn the_shipped_qfp_example_stays_quiet() {
    // A fine-pitch part on a shipped board is where this rule would fire
    // wrongly if the aperture were expanded rather than left as the pad.
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", "examples/blink.cypcb"])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        !said.contains("paste-clearance"),
        "the shipped board's pads are wide enough apart to stencil:\n{said}"
    );
}
