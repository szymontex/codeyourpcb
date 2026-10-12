//! A KiCad zone that is not a rectangle arrives as the ring it is drawn as.
//!
//! `cargo test -p cypcb-kicad --test a_zone_that_is_not_a_rectangle_is_kept_as_its_outline`
//!
//! The reader used to refuse every pour and rule area that was not an
//! axis-aligned rectangle, and refusing is right for a pour: a bounding box is
//! copper where the designer cut it away. It was also the whole zone gone, so
//! a keepout drawn round an antenna in the shape of an L kept nothing out, and
//! a board written back to KiCad lost the area. Measured on the KiCad
//! repository's own boards: 304 such zones in 112 files.
//!
//! What is held here: the corners arrive as the file states them, a keepout
//! is acted on whole, a pour is kept and is not filled - and says so, in the
//! words the refusal used - and the writer returns the ring it was given.

use cypcb_core::Point;
use cypcb_kicad::board_writer::write_board;
use cypcb_kicad::pcb_parser::parse_kicad_pcb_str;
use cypcb_world::components::zone::{Zone, ZoneKind};

fn board(zones: &str) -> String {
    format!(
        r#"(kicad_pcb (version 20240108) (generator "pcbnew")
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
{zones}
)
"#
    )
}

/// A rule area shaped as an L: 10 x 10 with the upper right quarter cut away,
/// the corner of the board at 100, 100.
const L_KEEPOUT: &str = r#"
  (zone
    (layer "F.Cu")
    (keepout (tracks not_allowed) (vias not_allowed) (pads not_allowed))
    (polygon
      (pts (xy 105 105) (xy 115 105) (xy 115 110) (xy 110 110) (xy 110 115) (xy 105 115))
    )
  )"#;

const L_POUR: &str = r#"
  (zone
    (net 1)
    (net_name "GND")
    (layer "B.Cu")
    (polygon
      (pts (xy 102 102) (xy 138 102) (xy 138 115) (xy 120 115) (xy 120 128) (xy 102 128))
    )
  )"#;

fn mm(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

#[test]
fn a_keepout_that_is_an_l_arrives_with_its_corners() {
    let mut parsed = parse_kicad_pcb_str(&board(L_KEEPOUT)).expect("the board reads");

    assert_eq!(parsed.metadata.outlined_zone_count, 1);
    assert_eq!(parsed.metadata.zone_count, 0, "it is not a rectangle");
    assert!(
        parsed.metadata.zone_refusals.is_empty(),
        "a keepout is carried whole and needs no excuse: {:?}",
        parsed.metadata.zone_refusals
    );

    let outlined = parsed.world.outlined_zones();
    assert_eq!(outlined.len(), 1);
    let (_, zone, outline) = &outlined[0];
    assert_eq!(zone.kind, ZoneKind::Keepout);
    assert_eq!(zone.layer_mask, 0b01);
    assert_eq!(
        outline.corners(),
        [
            mm(5.0, 25.0),
            mm(15.0, 25.0),
            mm(15.0, 20.0),
            mm(10.0, 20.0),
            mm(10.0, 15.0),
            mm(5.0, 15.0)
        ],
        "the corners are the file's, on the board's origin, with Y up as the board has it"
    );
    assert!(outline.contains(mm(7.0, 23.0)));
    assert!(!outline.contains(mm(13.0, 17.0)), "the notch is outside");
}

#[test]
fn a_zone_that_is_not_a_rectangle_is_not_among_the_rectangles() {
    let mut parsed = parse_kicad_pcb_str(&board(&(L_KEEPOUT.to_string() + L_POUR))).unwrap();
    assert!(
        parsed.world.zones().is_empty(),
        "a reader of zones() takes the box of the ring, and the box is not the zone"
    );
    assert_eq!(parsed.world.outlined_zones().len(), 2);
}

#[test]
fn a_pour_that_is_an_l_is_kept_and_not_filled_and_the_reader_says_so() {
    let mut parsed = parse_kicad_pcb_str(&board(L_POUR)).expect("the board reads");

    assert_eq!(parsed.metadata.zone_count, 0, "not a rectangle");
    assert_eq!(
        parsed.metadata.outlined_zone_count, 1,
        "but it is in the board"
    );
    assert_eq!(parsed.metadata.zone_refusals.len(), 1);
    let said = &parsed.metadata.zone_refusals[0];
    assert!(
        said.contains("the zone on net GND is a 6-point outline"),
        "{said}"
    );
    assert!(said.contains("not filled"), "{said}");
    assert!(
        said.contains("bounding box would put copper where the shape was drawn to avoid it"),
        "the reason to refuse the box travels with the warning: {said}"
    );

    let outlined = parsed.world.outlined_zones();
    let (_, zone, outline) = &outlined[0];
    assert_eq!(zone.kind, ZoneKind::CopperPour);
    assert_eq!(zone.layer_mask, 0b10);
    assert!(zone.net.is_some());
    assert_eq!(
        zone.name.as_deref(),
        Some("GND"),
        "named for its net, as a rectangle is"
    );
    assert_eq!(outline.corners().len(), 6);
}

#[test]
fn a_pour_with_no_net_is_refused_as_it_always_was() {
    let no_net = L_POUR.replace("(net 1)\n    (net_name \"GND\")", "");
    assert_ne!(no_net, L_POUR, "the net is out of the zone");
    let mut parsed = parse_kicad_pcb_str(&board(&no_net)).unwrap();
    assert_eq!(parsed.metadata.outlined_zone_count, 0);
    assert!(parsed.world.outlined_zones().is_empty());
    assert!(
        parsed.metadata.zone_refusals[0].contains("poured to no net"),
        "{:?}",
        parsed.metadata.zone_refusals
    );
}

#[test]
fn an_outline_that_encloses_no_area_is_refused() {
    let flat = L_KEEPOUT.replace(
        "(xy 105 105) (xy 115 105) (xy 115 110) (xy 110 110) (xy 110 115) (xy 105 115)",
        "(xy 105 105) (xy 110 105) (xy 115 105)",
    );
    assert_ne!(flat, L_KEEPOUT);
    let mut parsed = parse_kicad_pcb_str(&board(&flat)).unwrap();
    assert_eq!(parsed.metadata.outlined_zone_count, 0);
    assert!(parsed.world.outlined_zones().is_empty());
    assert!(parsed.metadata.zone_refusals[0].contains("encloses no area"));
}

#[test]
fn four_corners_that_are_not_a_rectangle_are_an_outline() {
    // The corners of a diamond share a bounding box with a rectangle.
    let diamond = L_KEEPOUT.replace(
        "(xy 105 105) (xy 115 105) (xy 115 110) (xy 110 110) (xy 110 115) (xy 105 115)",
        "(xy 110 105) (xy 115 110) (xy 110 115) (xy 105 110)",
    );
    let mut parsed = parse_kicad_pcb_str(&board(&diamond)).unwrap();
    assert_eq!(parsed.metadata.zone_count, 0);
    assert_eq!(parsed.metadata.outlined_zone_count, 1);
    let outlined = parsed.world.outlined_zones();
    assert!(outlined[0].2.contains(mm(10.0, 20.0)));
    assert!(
        !outlined[0].2.contains(mm(14.0, 24.0)),
        "the box corner is not in it"
    );
}

#[test]
fn control_a_rectangle_is_still_a_rectangle() {
    let rect = L_KEEPOUT.replace(
        "(xy 105 105) (xy 115 105) (xy 115 110) (xy 110 110) (xy 110 115) (xy 105 115)",
        "(xy 105 105) (xy 115 105) (xy 115 115) (xy 105 115)",
    );
    let mut parsed = parse_kicad_pcb_str(&board(&rect)).unwrap();
    assert_eq!(parsed.metadata.zone_count, 1);
    assert_eq!(parsed.metadata.outlined_zone_count, 0);
    assert_eq!(parsed.world.zones().len(), 1);
    assert!(parsed.world.outlined_zones().is_empty());
}

#[test]
fn the_metadata_of_a_board_with_no_outlined_zone_is_what_it_was() {
    let rect = L_KEEPOUT.replace(
        "(xy 105 105) (xy 115 105) (xy 115 110) (xy 110 110) (xy 110 115) (xy 105 115)",
        "(xy 105 105) (xy 115 105) (xy 115 115) (xy 105 115)",
    );
    let plain = serde_json::to_string(&parse_kicad_pcb_str(&board(&rect)).unwrap().metadata)
        .expect("the metadata is JSON");
    assert!(
        !plain.contains("outlined_zone_count"),
        "a board with no such zone says nothing of them: {plain}"
    );
    let outlined = serde_json::to_string(&parse_kicad_pcb_str(&board(L_KEEPOUT)).unwrap().metadata)
        .expect("the metadata is JSON");
    assert!(outlined.contains("\"outlined_zone_count\":1"), "{outlined}");
}

#[test]
fn the_ring_goes_back_to_kicad_as_it_came() {
    let mut parsed = parse_kicad_pcb_str(&board(&(L_KEEPOUT.to_string() + L_POUR))).unwrap();
    let text = write_board(&mut parsed.world, "cypcb");
    assert_eq!(text.matches("(zone").count(), 2, "one zone each: {text}");

    let mut again = parse_kicad_pcb_str(&text).expect("this project reads its own output");
    let before = parsed.world.outlined_zones();
    let after = again.world.outlined_zones();
    assert_eq!(after.len(), 2, "both are still outlines");
    for ((_, zone_a, outline_a), (_, zone_b, outline_b)) in before.iter().zip(&after) {
        assert_eq!(zone_a.kind, zone_b.kind);
        assert_eq!(
            outline_a.corners(),
            outline_b.corners(),
            "{:?}",
            zone_a.kind
        );
    }
    assert!(again.world.zones().is_empty());
}

#[test]
fn a_rectangle_goes_back_as_the_four_corners_of_its_box() {
    let mut world = cypcb_world::BoardWorld::new();
    world.set_board(
        "b".to_string(),
        (cypcb_core::Nm::from_mm(40.0), cypcb_core::Nm::from_mm(30.0)),
        2,
    );
    world.spawn_entity((Zone::keepout(
        cypcb_core::Rect::new(mm(5.0, 5.0), mm(15.0, 10.0)),
        0b01,
    ),));
    let text = write_board(&mut world, "cypcb");
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("(pts"))
        .expect("the zone has a polygon");
    assert_eq!(line.matches("(xy ").count(), 4, "{line}");
}
