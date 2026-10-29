//! A pour drawn as a polygon is measured as its ring by the three rules that
//! used to see rectangles only.
//!
//! `cargo test -p cypcb-drc --test a_polygon_pour_is_measured_as_the_ring_it_is`
//!
//! `EdgeClearanceRule`, `ZoneOverlapRule` and `MountingHoleClearanceRule` each
//! read the box a zone declares. For a pour drawn as an L the box is the notch
//! too: a corner of it hangs off a notched board, a rectangle standing in the
//! notch "overlaps" it, a hole in the notch "is in" it. Each rule now measures
//! the ring, in the way it measures a rectangle's box: a pour is the zone it
//! declares and not the copper the fill leaves; copper off the board reads as
//! zero; two pours share area or they do not, and meeting at an edge or a corner
//! is not sharing it; a hole is cut through copper or is not.
//!
//! Each has a notch case where the box gives the other answer, and a
//! rectangle control.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::rules::{EdgeClearanceRule, MountingHoleClearanceRule, ZoneOverlapRule};
use cypcb_drc::{DesignRules, DrcRule};
use cypcb_world::components::zone::{Zone, ZoneKind, ZoneOutline};
use cypcb_world::components::{
    BoardOutline, FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

fn p(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn rules() -> DesignRules {
    DesignRules::jlcpcb_2layer()
}

const TOP: u32 = 0b01;
const BOTTOM: u32 = 0b10;

fn ring(points: &[(f64, f64)]) -> ZoneOutline {
    ZoneOutline::new(points.iter().map(|&(x, y)| p(x, y)).collect()).expect("an outline")
}

/// An L, 20 x 20 from (5, 5), the upper right quarter cut away.
fn l_ring() -> ZoneOutline {
    ring(&[
        (5.0, 5.0),
        (25.0, 5.0),
        (25.0, 15.0),
        (15.0, 15.0),
        (15.0, 25.0),
        (5.0, 25.0),
    ])
}

fn pour_ring(world: &mut BoardWorld, ring: ZoneOutline, net: &str, mask: u32) {
    let net = world.intern_net(net);
    world.spawn_outlined_zone(Zone::copper_pour_for_net(Rect::default(), mask, net), ring);
}

fn pour_rect(world: &mut BoardWorld, rect: Rect, net: &str, mask: u32) {
    let net = world.intern_net(net);
    world.spawn_entity((Zone::copper_pour_for_net(rect, mask, net),));
}

// ---------------------------------------------------------------- edge

/// A 40 x 30 board whose outline is an L: the corner x > 20, y > 15 is cut away.
fn notched_board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("l".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let board = world.board_entity().unwrap();
    let outline = BoardOutline::new(vec![
        p(0.0, 0.0),
        p(40.0, 0.0),
        p(40.0, 15.0),
        p(20.0, 15.0),
        p(20.0, 30.0),
        p(0.0, 30.0),
    ])
    .expect("a ring");
    world.ecs_mut().entity_mut(board).insert(outline);
    world
}

/// The L-shaped pour tucked two millimetres inside the notched board, the same
/// shape as the board, whose box reaches into the board's notch.
fn tucked() -> ZoneOutline {
    ring(&[
        (2.0, 2.0),
        (38.0, 2.0),
        (38.0, 13.0),
        (18.0, 13.0),
        (18.0, 28.0),
        (2.0, 28.0),
    ])
}

#[test]
fn a_ring_tucked_inside_a_notched_board_is_clear_of_its_edge() {
    let mut world = notched_board();
    pour_ring(&mut world, tucked(), "GND", TOP);
    assert!(EdgeClearanceRule.check(&mut world, &rules()).is_empty());
}

#[test]
fn control_the_box_of_that_ring_hangs_into_the_notch() {
    let mut world = notched_board();
    pour_rect(
        &mut world,
        Rect::new(p(2.0, 2.0), p(38.0, 28.0)),
        "GND",
        TOP,
    );
    let found = EdgeClearanceRule.check(&mut world, &rules());
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn a_ring_a_tenth_of_a_millimetre_from_the_edge_is_reported_at_that_distance() {
    let mut world = notched_board();
    pour_ring(
        &mut world,
        ring(&[
            (0.1, 2.0),
            (38.0, 2.0),
            (38.0, 13.0),
            (18.0, 13.0),
            (18.0, 28.0),
            (0.1, 28.0),
        ]),
        "GND",
        TOP,
    );
    let found = EdgeClearanceRule.check(&mut world, &rules());
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].actual, Some(Nm::from_mm(0.1)));
    // Where it is reported: a corner of the ring on the near edge.
    assert_eq!(found[0].location.x, Nm::from_mm(0.1));
}

#[test]
fn a_ring_with_a_corner_off_the_board_reads_as_zero() {
    let mut world = notched_board();
    // The L's own box corner (30, 28) is off the board, in the notch.
    pour_ring(&mut world, l_ring_moved_into_the_notch(), "GND", TOP);
    let found = EdgeClearanceRule.check(&mut world, &rules());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].actual, Some(Nm::ZERO));
}

fn l_ring_moved_into_the_notch() -> ZoneOutline {
    ring(&[
        (22.0, 17.0),
        (32.0, 17.0),
        (32.0, 27.0),
        (22.0, 27.0),
        (22.0, 22.0),
        (27.0, 22.0),
    ])
}

/// A rectangular board, no outline: the four sides are the edge.
fn plain_board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("r".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    world
}

#[test]
fn on_a_rectangular_board_a_ring_is_measured_by_its_nearest_corner() {
    let mut world = plain_board();
    pour_ring(&mut world, l_ring(), "GND", TOP);
    assert!(
        EdgeClearanceRule.check(&mut world, &rules()).is_empty(),
        "five millimetres in"
    );

    let mut near = plain_board();
    pour_ring(
        &mut near,
        ring(&[
            (0.1, 5.0),
            (25.0, 5.0),
            (25.0, 15.0),
            (15.0, 15.0),
            (15.0, 25.0),
            (0.1, 25.0),
        ]),
        "GND",
        TOP,
    );
    let found = EdgeClearanceRule.check(&mut near, &rules());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].actual, Some(Nm::from_mm(0.1)));
}

#[test]
fn control_a_rectangle_pour_a_tenth_from_the_edge_is_the_same_report() {
    let mut world = plain_board();
    pour_rect(
        &mut world,
        Rect::new(p(0.1, 5.0), p(25.0, 25.0)),
        "GND",
        TOP,
    );
    let found = EdgeClearanceRule.check(&mut world, &rules());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].actual, Some(Nm::from_mm(0.1)));
}

#[test]
fn a_keepout_ring_is_not_copper_and_is_not_measured_against_the_edge() {
    let mut world = plain_board();
    world.spawn_outlined_zone(
        Zone::keepout(Rect::default(), TOP),
        ring(&[
            (0.0, 5.0),
            (25.0, 5.0),
            (25.0, 15.0),
            (15.0, 15.0),
            (15.0, 25.0),
            (0.0, 25.0),
        ]),
    );
    assert!(EdgeClearanceRule.check(&mut world, &rules()).is_empty());
}

// ------------------------------------------------------------- overlap

fn overlaps(world: &mut BoardWorld) -> usize {
    ZoneOverlapRule.check(world, &rules()).len()
}

#[test]
fn a_rectangle_in_the_notch_of_an_l_overlaps_the_box_and_not_the_ring() {
    let mut world = plain_board();
    pour_ring(&mut world, l_ring(), "GND", TOP);
    pour_rect(
        &mut world,
        Rect::new(p(17.0, 17.0), p(23.0, 23.0)),
        "SIG",
        TOP,
    );
    assert_eq!(overlaps(&mut world), 0, "the notch is not the L's copper");
}

#[test]
fn control_the_same_rectangle_against_the_box_of_the_ring_is_a_short() {
    let mut world = plain_board();
    pour_rect(
        &mut world,
        Rect::new(p(5.0, 5.0), p(25.0, 25.0)),
        "GND",
        TOP,
    );
    pour_rect(
        &mut world,
        Rect::new(p(17.0, 17.0), p(23.0, 23.0)),
        "SIG",
        TOP,
    );
    assert_eq!(overlaps(&mut world), 1);
}

#[test]
fn a_rectangle_over_the_foot_of_the_l_is_a_short_where_the_two_share_area() {
    let mut world = plain_board();
    pour_ring(&mut world, l_ring(), "GND", TOP);
    pour_rect(
        &mut world,
        Rect::new(p(20.0, 8.0), p(30.0, 12.0)),
        "SIG",
        TOP,
    );
    let found = ZoneOverlapRule.check(&mut world, &rules());
    assert_eq!(found.len(), 1);
    // The middle of the box the shared pieces share: inside the foot.
    assert!(
        found[0].location.x.0 >= Nm::from_mm(20.0).0
            && found[0].location.x.0 <= Nm::from_mm(25.0).0
    );
    assert!(
        found[0].location.y.0 >= Nm::from_mm(8.0).0 && found[0].location.y.0 <= Nm::from_mm(12.0).0
    );
}

#[test]
fn meeting_along_an_edge_is_not_sharing_area_for_a_ring() {
    // The rectangle fills the notch against the L's two walls: it shares an edge
    // with the ring, 5 mm of it, and no area.
    let mut world = plain_board();
    pour_ring(&mut world, l_ring(), "GND", TOP);
    pour_rect(
        &mut world,
        Rect::new(p(15.0, 15.0), p(20.0, 20.0)),
        "SIG",
        TOP,
    );
    assert_eq!(overlaps(&mut world), 0);
}

#[test]
fn control_two_rectangles_that_meet_along_an_edge_are_not_a_short() {
    let mut world = plain_board();
    pour_rect(
        &mut world,
        Rect::new(p(5.0, 5.0), p(15.0, 15.0)),
        "GND",
        TOP,
    );
    pour_rect(
        &mut world,
        Rect::new(p(15.0, 5.0), p(25.0, 15.0)),
        "SIG",
        TOP,
    );
    assert_eq!(overlaps(&mut world), 0);
}

#[test]
fn meeting_at_one_corner_is_not_sharing_area() {
    let mut world = plain_board();
    pour_ring(&mut world, l_ring(), "GND", TOP);
    // A triangle whose corner is the L's corner (25, 15), and nothing else.
    pour_ring(
        &mut world,
        ring(&[(25.0, 15.0), (30.0, 15.0), (27.0, 20.0)]),
        "SIG",
        TOP,
    );
    assert_eq!(overlaps(&mut world), 0);
}

#[test]
fn two_rings_that_share_area_are_a_short() {
    let mut world = plain_board();
    pour_ring(&mut world, l_ring(), "GND", TOP);
    pour_ring(
        &mut world,
        ring(&[(20.0, 10.0), (30.0, 10.0), (25.0, 20.0)]),
        "SIG",
        TOP,
    );
    assert_eq!(overlaps(&mut world), 1);
}

#[test]
fn two_rings_whose_boxes_overlap_in_the_notch_only_are_not() {
    let mut world = plain_board();
    pour_ring(&mut world, l_ring(), "GND", TOP);
    // An L of its own, in the notch of the first, its box in the first's box.
    pour_ring(
        &mut world,
        ring(&[
            (17.0, 17.0),
            (23.0, 17.0),
            (23.0, 19.0),
            (19.0, 19.0),
            (19.0, 23.0),
            (17.0, 23.0),
        ]),
        "SIG",
        TOP,
    );
    assert_eq!(overlaps(&mut world), 0);
}

#[test]
fn the_same_net_or_another_layer_is_not_a_short_for_a_ring() {
    let mut same = plain_board();
    pour_ring(&mut same, l_ring(), "GND", TOP);
    pour_ring(
        &mut same,
        ring(&[(20.0, 10.0), (30.0, 10.0), (25.0, 20.0)]),
        "GND",
        TOP,
    );
    assert_eq!(overlaps(&mut same), 0, "one plane drawn twice");

    let mut other = plain_board();
    pour_ring(&mut other, l_ring(), "GND", TOP);
    pour_ring(
        &mut other,
        ring(&[(20.0, 10.0), (30.0, 10.0), (25.0, 20.0)]),
        "SIG",
        BOTTOM,
    );
    assert_eq!(overlaps(&mut other), 0, "the other side of the board");
}

// ---------------------------------------------------------- mounting hole

/// A board with a mounting hole at `(x, y)`: a 3.2 mm drill and no copper.
fn board_with_hole(x: f64, y: f64) -> BoardWorld {
    let mut world = plain_board();
    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(
        Footprint {
            placement_origin: None,
            name: "screw".to_string(),
            pads: vec![PadDef {
                hole_offset: None,
                number: "1".to_string(),
                shape: PadShape::Circle,
                position: Point::from_mm(0.0, 0.0),
                size: (Nm::from_mm(3.2), Nm::from_mm(3.2)),
                drill: Some(Nm::from_mm(3.2)),
                slot: None,
                layers: Vec::new(),
                mask_margin: None,
                rotation: Rotation::ZERO,
            }],
            ..base
        }
        .with_courtyard_holding_its_pads(),
    );
    world.set_footprints(library);
    world.spawn_component(
        RefDes::new("H1"),
        Value::new(""),
        Position::from_mm(x, y),
        Rotation::ZERO,
        FootprintRef::new("screw"),
        NetConnections::new(),
    );
    world
}

fn hole_faults(world: &mut BoardWorld) -> Vec<cypcb_drc::DrcViolation> {
    MountingHoleClearanceRule.check(world, &rules())
}

#[test]
fn a_hole_in_the_notch_of_an_l_is_clear_of_the_ring() {
    // 5 mm from the notch's wall: well clear, though inside the box.
    let mut world = board_with_hole(20.0, 20.0);
    pour_ring(&mut world, l_ring(), "GND", TOP);
    assert!(hole_faults(&mut world).is_empty());
}

#[test]
fn control_the_same_hole_in_the_box_of_the_ring_is_cut_through_copper() {
    let mut world = board_with_hole(20.0, 20.0);
    pour_rect(
        &mut world,
        Rect::new(p(5.0, 5.0), p(25.0, 25.0)),
        "GND",
        TOP,
    );
    assert_eq!(hole_faults(&mut world).len(), 1);
}

#[test]
fn a_hole_a_tenth_of_a_millimetre_from_the_ring_is_reported_at_that_gap() {
    // The wall of the notch is x = 15; the hole's wall is 1.6 from its centre.
    let mut world = board_with_hole(16.7, 20.0);
    pour_ring(&mut world, l_ring(), "GND", TOP);
    let found = hole_faults(&mut world);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].actual, Some(Nm::from_mm(0.1)));
    assert!(found[0].message.contains("H1"), "{}", found[0].message);
}

#[test]
fn a_hole_inside_the_ring_is_cut_through_copper() {
    let mut world = board_with_hole(10.0, 10.0);
    pour_ring(&mut world, l_ring(), "GND", TOP);
    let found = hole_faults(&mut world);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].actual, Some(Nm::ZERO));
}

#[test]
fn a_keepout_ring_is_no_copper_for_a_hole() {
    let mut world = board_with_hole(10.0, 10.0);
    world.spawn_outlined_zone(Zone::keepout(Rect::default(), TOP), l_ring());
    assert!(hole_faults(&mut world).is_empty());
}

#[test]
fn a_flex_region_stays_where_it_was() {
    // The rectangles keep their path: a flex region is not a keepout, and the rule
    // has always measured it.
    let mut world = board_with_hole(10.0, 10.0);
    world.spawn_entity((Zone {
        kind: ZoneKind::Flex,
        ..Zone::keepout(Rect::new(p(5.0, 5.0), p(15.0, 15.0)), TOP | BOTTOM)
    },));
    assert_eq!(hole_faults(&mut world).len(), 1);
    let _ = Layer::TopCopper;
}
