//! A part is in a keepout that is a polygon when its centre is in the polygon.
//!
//! `cargo test -p cypcb-drc --test a_keepout_that_is_a_polygon_holds_its_shape`
//!
//! `KeepoutRule` asks one thing of a keepout: is the centre of a part inside
//! it. For a rectangle that was a box test. A keepout read from KiCad as an L
//! was not in the board at all - the reader refused it - so a part stood in
//! the corner of the L and nothing was said. With the ring in the model the
//! question is the same and the answer is exact: on an edge or a corner is
//! inside, as it is for a rectangle, and the notch of a concave outline is
//! not.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{DrcRule, KeepoutRule};
use cypcb_world::components::zone::{Zone, ZoneOutline};
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation, Value};
use cypcb_world::BoardWorld;

fn p(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

/// A board with an L-shaped keepout, 10 x 10 with the upper right quarter
/// (5..10 by 5..10) cut away, and one part at `(x, y)`.
fn board_with_a_part_at(x: f64, y: f64) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let l = ZoneOutline::new(vec![
        p(0.0, 0.0),
        p(10.0, 0.0),
        p(10.0, 5.0),
        p(5.0, 5.0),
        p(5.0, 10.0),
        p(0.0, 10.0),
    ])
    .expect("an L");
    world.spawn_outlined_zone(Zone::keepout(Rect::default(), 0b11).with_name("antenna"), l);
    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(x, y),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );
    world
}

fn violations_with_a_part_at(x: f64, y: f64) -> usize {
    let mut world = board_with_a_part_at(x, y);
    KeepoutRule
        .check(&mut world, &DesignRules::jlcpcb_2layer())
        .len()
}

#[test]
fn a_part_inside_the_polygon_is_in_the_keepout() {
    assert_eq!(violations_with_a_part_at(2.0, 2.0), 1);
    assert_eq!(violations_with_a_part_at(8.0, 2.0), 1, "the foot of the L");
    assert_eq!(violations_with_a_part_at(2.0, 8.0), 1, "the stem of the L");
}

#[test]
fn a_part_on_an_edge_is_in_the_keepout() {
    assert_eq!(violations_with_a_part_at(5.0, 0.0), 1, "the bottom edge");
    assert_eq!(violations_with_a_part_at(0.0, 5.0), 1, "the left edge");
    assert_eq!(violations_with_a_part_at(10.0, 3.0), 1, "the right edge");
    assert_eq!(
        violations_with_a_part_at(7.0, 5.0),
        1,
        "the edge of the notch"
    );
}

#[test]
fn a_part_on_a_corner_is_in_the_keepout() {
    assert_eq!(violations_with_a_part_at(0.0, 0.0), 1);
    assert_eq!(
        violations_with_a_part_at(5.0, 5.0),
        1,
        "the corner that turns inward"
    );
}

#[test]
fn a_part_in_the_notch_of_a_concave_outline_is_not() {
    assert_eq!(
        violations_with_a_part_at(8.0, 8.0),
        0,
        "in the notch, inside the box"
    );
    assert_eq!(violations_with_a_part_at(6.0, 6.0), 0);
}

#[test]
fn a_part_outside_the_box_is_not() {
    assert_eq!(violations_with_a_part_at(-1.0, 5.0), 0);
    assert_eq!(violations_with_a_part_at(20.0, 20.0), 0);
}

#[test]
fn the_violation_names_the_zone_and_the_part() {
    let mut world = board_with_a_part_at(2.0, 2.0);
    let found = KeepoutRule.check(&mut world, &DesignRules::jlcpcb_2layer());
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("R1"), "{}", found[0].message);
    assert!(found[0].message.contains("antenna"), "{}", found[0].message);
}

#[test]
fn control_a_keepout_that_is_a_rectangle_is_asked_the_same_way() {
    for (x, y, inside) in [(8.0, 8.0, true), (10.0, 10.0, true), (11.0, 5.0, false)] {
        let mut world = BoardWorld::new();
        world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
        world.spawn_entity((Zone::keepout(Rect::new(p(0.0, 0.0), p(10.0, 10.0)), 0b11),));
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(x, y),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );
        let found = KeepoutRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert_eq!(found.len(), usize::from(inside), "({x}, {y})");
    }
}
