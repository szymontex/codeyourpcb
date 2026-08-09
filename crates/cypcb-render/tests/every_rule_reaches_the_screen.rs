//! What the checker finds has to arrive in the browser, with a line to jump to.
//!
//! `cargo test -p cypcb-render --features native --test every_rule_reaches_the_screen`
//!
//! The viewer draws its error panel from the snapshot's violations, and each
//! one carries a kind, a place on the board and the line of the definition it
//! is about. Rules added since that panel was written have never been checked
//! against it: a violation that arrives with no line lands the reader on line
//! 1, and one whose kind the panel does not recognise is a row nobody can act
//! on.
//!
//! This drives the engine the way the browser does - `load_source`, then the
//! snapshot - and asks the newest rules for their answers.

use cypcb_render::PcbEngine;

/// A design that breaks three of the rules added most recently.
const BOARD: &str = r#"version 1

board panel {
    size 60mm x 40mm
    layers 2
}

interface I2C {
    pin SDA
    pin SCL
}

component J1 connector "PIN-HDR-1x2" {
    at 10mm, 20mm
}

component U1 ic "SOIC-8" {
    value "USB"
    at 40mm, 20mm
}

net USB_DP {
    J1.1
    U1.1
}

net USB_DM {
    J1.2
    U1.2
}

diffpair USB {
    USB_DP
    USB_DM
}

trace USB_DP {
    layer Top
    width 0.25mm
    path 10mm,19mm -> 40mm,19mm
}

trace USB_DM {
    layer Top
    width 0.25mm
    path 10mm,21mm -> 55mm,21mm -> 40mm,21mm
}
"#;

fn violations(source: &str) -> Vec<serde_json::Value> {
    let mut engine = PcbEngine::new();
    engine.load_source(source);
    let snapshot: serde_json::Value =
        serde_json::from_str(&engine.get_snapshot()).expect("the snapshot is JSON");
    snapshot["violations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn a_differential_pair_skew_reaches_the_browser() {
    let found = violations(BOARD);

    let skew = found
        .iter()
        .find(|v| v["kind"] == "diff-pair-skew")
        .unwrap_or_else(|| {
            panic!("the halves differ by 30mm and nothing reached the snapshot: {found:#?}")
        });

    let message = skew["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("USB_DP") && message.contains("USB_DM"),
        "the panel shows this text, so it has to name both halves: {message}"
    );
}

#[test]
fn every_violation_the_panel_shows_has_a_place_on_the_board() {
    // The panel puts a marker where the fault is. A violation at the origin is
    // a marker in the corner of every board, which is worse than none.
    let found = violations(BOARD);
    assert!(!found.is_empty(), "the fixture is supposed to break rules");

    let at_origin: Vec<&str> = found
        .iter()
        .filter(|v| v["x_nm"] == 0 && v["y_nm"] == 0)
        .filter_map(|v| v["kind"].as_str())
        .collect();

    assert!(
        at_origin.is_empty(),
        "these arrive pointing at 0,0: {at_origin:?}"
    );
}

#[test]
fn a_violation_the_model_can_place_carries_its_line() {
    // `line` is what the editor jumps to. It is optional - a rule about the
    // board as a whole has no definition to point at - but a rule about a part
    // or a trace does, and those are most of them.
    let found = violations(BOARD);

    let placed = found
        .iter()
        .filter(|v| v.get("line").and_then(|l| l.as_u64()).is_some())
        .count();

    assert!(
        placed > 0,
        "not one violation knows which line it is about: {found:#?}"
    );
}
