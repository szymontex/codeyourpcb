//! A KiCad rule area that forbids nothing is not a keepout.
//!
//! `cargo test -p cypcb-kicad --test a_rule_area_that_allows_everything_keeps_nothing_out`
//!
//! KiCad's rule area has five switches, tracks, vias, pads, copper pour and
//! footprints, each `allowed` or `not_allowed`, and it is also what placement
//! rules are drawn on. The reader took any `(keepout ...)` node as a keepout and
//! never read the switches, so a placement area was a place nothing may enter:
//! 127 keepout violations on one board of KiCad's own tests, none of them real.
//! Measured on the 112 boards of that suite: 71 such nodes, 30 of which allow all
//! five.
//!
//! What is held here: an area that allows all five is left out, and says so; an
//! area that forbids anything is still carried, as it was. The model has one kind
//! of keepout and no per-kind switches, so a partial area is carried as a whole
//! keepout - a known gap, kept visible by the controls below.

use cypcb_kicad::pcb_parser::{parse_kicad_pcb_str, KicadPcbParseResult};
use cypcb_world::components::zone::{Zone, ZoneKind};

fn board(zone: &str) -> String {
    format!(
        r#"(kicad_pcb (version 20240108) (generator "pcbnew")
  (general (thickness 1.6))
  (paper "A4")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))
{zone}
)
"#
    )
}

/// The shape of a placement area in KiCad's own tests: an L, five switches all
/// `allowed`, a placement block, on both faces.
fn area(
    layer: &str,
    tracks: &str,
    vias: &str,
    pads: &str,
    pour: &str,
    footprints: &str,
    placement: bool,
) -> String {
    let placement = if placement {
        r#"(placement (enabled yes) (sheetname "/CH2/"))"#
    } else {
        ""
    };
    format!(
        r#"(zone (net 0) (net_name "") (layer "{layer}") (name "area")
  (keepout (tracks {tracks}) (vias {vias}) (pads {pads}) (copperpour {pour}) (footprints {footprints}))
  {placement}
  (polygon (pts (xy 105 105) (xy 115 105) (xy 115 110) (xy 110 110) (xy 110 115) (xy 105 115))))"#
    )
}

fn read(zone: &str) -> KicadPcbParseResult {
    parse_kicad_pcb_str(&board(zone)).expect("the board reads")
}

fn zones(parsed: &mut KicadPcbParseResult) -> Vec<Zone> {
    let ecs = parsed.world.ecs_mut();
    let mut query = ecs.query::<&Zone>();
    query.iter(ecs).cloned().collect()
}

#[test]
fn a_placement_area_that_allows_all_five_is_left_out_and_says_so() {
    let mut parsed = read(&area(
        "F&B.Cu", "allowed", "allowed", "allowed", "allowed", "allowed", true,
    ));
    assert!(zones(&mut parsed).is_empty(), "no keepout");
    assert_eq!(parsed.metadata.outlined_zone_count, 0);
    assert_eq!(parsed.metadata.zone_refusals.len(), 1);
    assert!(
        parsed.metadata.zone_refusals[0].contains("keeps nothing out"),
        "{:?}",
        parsed.metadata.zone_refusals
    );
}

#[test]
fn a_rule_area_with_no_placement_block_that_allows_all_five_is_left_out_too() {
    for layer in ["F.Cu", "B.Cu"] {
        let mut parsed = read(&area(
            layer, "allowed", "allowed", "allowed", "allowed", "allowed", false,
        ));
        assert!(zones(&mut parsed).is_empty(), "{layer}");
        assert_eq!(parsed.metadata.zone_refusals.len(), 1, "{layer}");
    }
}

#[test]
fn control_an_area_that_forbids_everything_is_a_keepout() {
    let mut parsed = read(&area(
        "F.Cu",
        "not_allowed",
        "not_allowed",
        "not_allowed",
        "not_allowed",
        "not_allowed",
        false,
    ));
    assert!(parsed.metadata.zone_refusals.is_empty());
    assert_eq!(parsed.metadata.outlined_zone_count, 1);
    let zones = zones(&mut parsed);
    assert_eq!(zones.len(), 1);
    assert_eq!(zones[0].kind, ZoneKind::Keepout);
}

#[test]
fn control_an_area_that_forbids_one_thing_is_still_carried_as_a_keepout() {
    // The model has one kind of keepout. Each of the five alone keeps the area.
    for which in 0..5 {
        let mut switches = ["allowed"; 5];
        switches[which] = "not_allowed";
        let [tracks, vias, pads, pour, footprints] = switches;
        let mut parsed = read(&area("B.Cu", tracks, vias, pads, pour, footprints, true));
        assert!(parsed.metadata.zone_refusals.is_empty(), "switch {which}");
        let zones = zones(&mut parsed);
        assert_eq!(zones.len(), 1, "switch {which}");
        assert_eq!(zones[0].kind, ZoneKind::Keepout, "switch {which}");
    }
}

#[test]
fn control_a_rectangular_area_that_allows_everything_is_left_out_as_well() {
    let rectangle = r#"(zone (net 0) (net_name "") (layer "F.Cu")
  (keepout (tracks allowed) (vias allowed) (pads allowed) (copperpour allowed) (footprints allowed))
  (polygon (pts (xy 105 105) (xy 115 105) (xy 115 115) (xy 105 115))))"#;
    let mut parsed = read(rectangle);
    assert!(zones(&mut parsed).is_empty());
    assert_eq!(parsed.metadata.zone_count, 0);
    assert_eq!(parsed.metadata.zone_refusals.len(), 1);
}
