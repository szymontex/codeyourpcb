//! The snapshot the viewer draws from carries the ring of a polygon zone.
//!
//! `cargo test -p cypcb-render --features native --test a_zone_that_is_not_a_rectangle_reaches_the_screen_as_its_ring`
//!
//! A keepout read from KiCad as an L is in the board with its corners. The
//! snapshot sends the zone with its `bounds` as always and its corners besides,
//! so the screen can draw the L and not the box round it. A rectangle sends
//! what it always sent, byte for byte: the field is left out.

use cypcb_render::PcbEngine;

fn board(points: &str) -> String {
    format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\")
           (general (thickness 1.6)) (paper \"A4\")
           (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
           (net 0 \"\")
           (gr_rect (start 100 100) (end 140 130) (layer \"Edge.Cuts\") (width 0.05))
           (zone (layer \"F.Cu\")
             (keepout (tracks not_allowed) (vias not_allowed) (pads not_allowed))
             (polygon (pts {points}))))"
    )
}

const L: &str = "(xy 105 105) (xy 115 105) (xy 115 110) (xy 110 110) (xy 110 115) (xy 105 115)";
const RECTANGLE: &str = "(xy 105 105) (xy 115 105) (xy 115 115) (xy 105 115)";

fn zones_json(points: &str) -> Vec<String> {
    let mut engine = PcbEngine::new();
    let said = engine.load_kicad(&board(points));
    assert_eq!(said, "", "the board loads");
    engine
        .build_snapshot()
        .zones
        .iter()
        .map(|zone| serde_json::to_string(zone).expect("a zone is JSON"))
        .collect()
}

#[test]
fn a_zone_that_is_an_l_is_sent_with_its_corners() {
    let zones = zones_json(L);
    assert_eq!(zones.len(), 1, "one zone: {zones:?}");
    // Y is up on the board: file y 105 is 25mm from the bottom.
    assert!(
        zones[0].contains(
            "\"outline\":[[5000000,25000000],[15000000,25000000],[15000000,20000000],\
             [10000000,20000000],[10000000,15000000],[5000000,15000000]]"
        ),
        "{}",
        zones[0]
    );
    assert!(zones[0].contains("\"kind\":\"keepout\""), "{}", zones[0]);
    assert!(
        zones[0].contains("\"bounds\":[5000000,15000000,15000000,25000000]"),
        "the box is still sent: {}",
        zones[0]
    );
}

#[test]
fn control_a_rectangle_is_sent_without_an_outline() {
    let zones = zones_json(RECTANGLE);
    assert_eq!(zones.len(), 1);
    assert!(!zones[0].contains("outline"), "{}", zones[0]);
    assert!(
        zones[0].contains("\"bounds\":[5000000,15000000,15000000,25000000]"),
        "{}",
        zones[0]
    );
}
