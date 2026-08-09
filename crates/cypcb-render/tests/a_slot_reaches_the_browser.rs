//! A slot has to survive the trip to the screen, not only to the fab.
//!
//! `cargo test -p cypcb-render --test a_slot_reaches_the_browser`
//!
//! The snapshot is the whole of what the browser knows about a board. It
//! carried one drill number per pad, which for a slot is its **narrow**
//! dimension - so the engine imported a 2.4x1.0mm slot correctly, wrote it
//! correctly into the drill file, and then told the viewer about a 1mm round
//! hole. The renderer drew what it was told.
//!
//! This is the same path the browser takes: KiCad text in, snapshot out.

use cypcb_render::PcbEngine;

/// A board with one slotted pad and one round one, in the same footprint.
const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator "hand-written-test")
  (general (thickness 1.6))
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (net 1 "VCC")
  (net 2 "GND")
  (gr_rect (start 0 0) (end 30 20) (layer "Edge.Cuts") (width 0.05))
  (footprint "Connector:USB_C"
    (at 20 12)
    (property "Reference" "J1")
    (pad "1" thru_hole circle (at 0 -1.27) (size 1.8 1.8) (drill 0.9) (layers "*.Cu" "*.Mask") (net 1 "VCC"))
    (pad "2" thru_hole oval (at 0 1.27) (size 3.2 1.8) (drill oval 2.4 1.0) (layers "*.Cu" "*.Mask") (net 2 "GND"))
  )
)
"#;

fn snapshot() -> String {
    let mut engine = PcbEngine::new();
    let report = engine.load_kicad(BOARD);
    assert!(
        !report.to_lowercase().contains("error"),
        "the board loads: {report}"
    );
    engine.get_snapshot()
}

#[test]
fn the_snapshot_carries_both_of_the_slots_dimensions() {
    let json = snapshot();

    assert!(
        json.contains("\"slot_nm\":[2400000,1000000]"),
        "the slot reaches the browser whole: {}",
        pads_of(&json)
    );
}

#[test]
fn the_drill_stays_the_narrow_dimension() {
    // What the viewer sizes a plating ring from and what every rule means by
    // a drill. Taking 2.4mm here would draw a hole wider than the pad.
    let json = snapshot();

    assert!(json.contains("\"drill_nm\":1000000"), "{}", pads_of(&json));
}

#[test]
fn a_round_hole_says_nothing_about_slots() {
    // The field is absent rather than null for a round hole, so a snapshot
    // written before slots existed still reads and the common case costs no
    // bytes. Pad 1 is `(drill 0.9)`.
    let json = snapshot();

    assert!(
        json.contains("\"drill_nm\":900000,\"") || json.contains("\"drill_nm\":900000}"),
        "the round hole is there: {}",
        pads_of(&json)
    );
    assert_eq!(
        json.matches("\"slot_nm\"").count(),
        1,
        "only the slotted pad names a slot: {}",
        pads_of(&json)
    );
}

/// The pad fragment of the snapshot, for a failure that can be read.
fn pads_of(json: &str) -> String {
    match json.find("\"pads\"") {
        Some(at) => json[at..(at + 600).min(json.len())].to_string(),
        None => json.chars().take(600).collect(),
    }
}
