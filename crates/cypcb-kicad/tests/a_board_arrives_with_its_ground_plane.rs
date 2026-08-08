//! A ground plane is the largest thing on a real board, and the importer read
//! none of them.
//!
//! `cargo test -p cypcb-kicad --test a_board_arrives_with_its_ground_plane`
//!
//! `(zone ...)` appeared nowhere in `pcb_parser.rs` - the file dispatched
//! `footprint`, `segment`, `via` and the `gr_*` outline shapes, and walked
//! past every pour. So a two-layer board with a GND plane, which is close to
//! every two-layer board anybody makes, arrived here with no plane at all:
//!
//! - the router treated the whole poured area as free copper to route through,
//! - the checker measured clearances against copper that was not in the model,
//! - and the exported Gerber shipped a board with no ground.
//!
//! None of the five benchmark fixtures carries a zone either, so the router
//! has never been measured on a board with a plane. That is worth knowing
//! separately from this fix.
//!
//! What can be carried is carried exactly. This crate's `Zone` is a rectangle
//! and KiCad's is a polygon, so a rectangular pour comes across as itself and
//! anything else is refused **by name**. A bounding box is not a conservative
//! reading of an L-shaped pour: it is copper where the designer deliberately
//! left none, in exactly the places the shape was drawn to avoid.

use cypcb_kicad::parse_kicad_pcb;
use cypcb_world::components::zone::ZoneKind;

use std::io::Write;

/// Header, plus a rectangular GND pour on the bottom layer.
const BOARD_WITH_A_PLANE: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew")

  (general (thickness 1.6))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (44 "Edge.Cuts" user)
  )

  (net 0 "")
  (net 1 "GND")
  (net 2 "SIG")

  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))

  (footprint "Connector:Conn_01x02"
    (layer "F.Cu")
    (at 110 110)
    (property "Reference" "J1")
    (property "Value" "conn")
    (pad "1" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 1 "GND"))
    (pad "2" thru_hole oval (at 0 2.54) (size 1.7 1.7) (drill 1.0) (layers "*.Cu") (net 2 "SIG"))
  )

  (zone
    (net 1)
    (net_name "GND")
    (layer "B.Cu")
    (hatch edge 0.5)
    (connect_pads (clearance 0.5))
    (min_thickness 0.25)
    (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
    (polygon
      (pts (xy 102 102) (xy 138 102) (xy 138 128) (xy 102 128))
    )
  )
)
"#;

/// The same board, with the pour cut around a corner - an L, not a rectangle.
const BOARD_WITH_AN_L_SHAPED_PLANE: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew")

  (general (thickness 1.6))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (44 "Edge.Cuts" user)
  )

  (net 0 "")
  (net 1 "GND")

  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))

  (zone
    (net 1)
    (net_name "GND")
    (layer "B.Cu")
    (polygon
      (pts (xy 102 102) (xy 138 102) (xy 138 115) (xy 120 115) (xy 120 128) (xy 102 128))
    )
  )
)
"#;

/// A rule area: copper kept out rather than poured.
const BOARD_WITH_A_RULE_AREA: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew")

  (general (thickness 1.6))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (44 "Edge.Cuts" user)
  )

  (net 0 "")

  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))

  (zone
    (layer "F.Cu")
    (keepout (tracks not_allowed) (vias not_allowed) (pads not_allowed))
    (polygon
      (pts (xy 105 105) (xy 115 105) (xy 115 115) (xy 105 115))
    )
  )
)
"#;

fn parse(who: &str, source: &str) -> cypcb_kicad::KicadPcbParseResult {
    let dir = std::env::temp_dir().join("cypcb-kicad-zones");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let path = dir.join(format!("{who}.kicad_pcb"));
    let mut file = std::fs::File::create(&path).expect("the board is writable");
    file.write_all(source.as_bytes())
        .expect("the board is written");
    drop(file);

    parse_kicad_pcb(&path).unwrap_or_else(|e| panic!("{who} must parse: {e:?}"))
}

#[test]
fn a_rectangular_ground_plane_arrives_with_the_board() {
    let mut result = parse("plane", BOARD_WITH_A_PLANE);

    assert_eq!(
        result.metadata.zone_count, 1,
        "the file carries a ground plane and the board came back without it. \
         Refusals: {:?}",
        result.metadata.zone_refusals
    );
    assert!(
        result.metadata.zone_refusals.is_empty(),
        "a plain rectangular pour needs no excuse: {:?}",
        result.metadata.zone_refusals
    );

    let zones = result.world.zones();
    assert_eq!(zones.len(), 1, "one pour on the board");
    let (_, zone) = &zones[0];

    assert_eq!(zone.kind, ZoneKind::CopperPour);
    assert!(zone.net.is_some(), "a pour with no net cannot be filled");

    // The board origin is at 100, 100, so the pour's 102..138 by 102..128 in
    // file coordinates is 2..38 by 2..28 on the board.
    assert_eq!(zone.bounds.min.x.to_mm(), 2.0);
    assert_eq!(zone.bounds.min.y.to_mm(), 2.0);
    assert_eq!(zone.bounds.max.x.to_mm(), 38.0);
    assert_eq!(zone.bounds.max.y.to_mm(), 28.0);

    // Bottom copper only. A plane put on the wrong layer is worse than none.
    assert_eq!(
        zone.layer_mask, 0b10,
        "the file says B.Cu and nothing else, got mask {:#b}",
        zone.layer_mask
    );
}

#[test]
fn a_pour_this_importer_cannot_hold_is_refused_by_name() {
    let mut result = parse("l-shaped", BOARD_WITH_AN_L_SHAPED_PLANE);

    assert_eq!(
        result.metadata.zone_count, 0,
        "an L-shaped pour is not a rectangle and must not be carried as one"
    );
    assert_eq!(
        result.metadata.zone_refusals.len(),
        1,
        "the refusal has to be reported, or the plane is lost in silence"
    );

    let why = &result.metadata.zone_refusals[0];
    assert!(
        why.contains("GND"),
        "the reader has to be told which pour was left out: {why}"
    );
    assert!(why.contains("6-point"), "and what was wrong with it: {why}");

    // And nothing reached the board. A bounding box here would put copper
    // across the corner the designer cut away.
    assert_eq!(result.world.zones().len(), 0);
}

#[test]
fn a_rule_area_arrives_as_a_keepout() {
    let mut result = parse("rule-area", BOARD_WITH_A_RULE_AREA);

    assert_eq!(
        result.metadata.zone_count, 1,
        "{:?}",
        result.metadata.zone_refusals
    );
    let zones = result.world.zones();
    let (_, zone) = &zones[0];

    assert_eq!(
        zone.kind,
        ZoneKind::Keepout,
        "a KiCad rule area keeps copper out; carrying it as a pour would fill \
         the one region on the board that must stay empty"
    );
    assert!(zone.net.is_none(), "a keepout is on no net");
    assert_eq!(zone.bounds.min.x.to_mm(), 5.0);
    assert_eq!(zone.bounds.max.x.to_mm(), 15.0);
}

#[test]
fn a_board_with_no_zones_still_parses_and_says_so() {
    // The half that must not change: five benchmark fixtures carry no zone,
    // and they have to keep importing exactly as they did.
    let mut result = parse(
        "no-zones",
        BOARD_WITH_A_PLANE
            .replace("  (zone", "  (unused_zone")
            .as_str(),
    );

    assert_eq!(result.metadata.zone_count, 0);
    assert!(result.metadata.zone_refusals.is_empty());
    assert_eq!(result.world.zones().len(), 0);
    assert_eq!(
        result.metadata.component_count, 1,
        "the rest of the board still arrives"
    );
}
