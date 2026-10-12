//! A zone drawn as a polygon is its ring of corners, and not the box round it.
//!
//! `cargo test -p cypcb-world --test a_zone_that_is_not_a_rectangle_is_not_its_box`
//!
//! `Zone` is a rectangle, and a KiCad pour or rule area is a polygon. The
//! reader refused every one that was not an axis-aligned rectangle, because a
//! bounding box is copper where the designer cut it away. Carrying the ring in
//! the model has the same trap one step on: the box is right there in
//! `Zone::bounds`, and every reader that takes it is the bounding box again.
//! So the model keeps the two apart. `zones()` is the zones that are exactly
//! their bounds and nothing else, `outlined_zones()` is the others with their
//! corners, and a zone with an outline is never in the first.

use cypcb_core::{Nm, Point, Rect};
use cypcb_world::components::zone::{Zone, ZoneKind, ZoneOutline};
use cypcb_world::BoardWorld;

fn p(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

/// An L: 10 x 10 with the upper right quarter cut away.
fn l_shape() -> ZoneOutline {
    ZoneOutline::new(vec![
        p(0.0, 0.0),
        p(10.0, 0.0),
        p(10.0, 5.0),
        p(5.0, 5.0),
        p(5.0, 10.0),
        p(0.0, 10.0),
    ])
    .expect("an L encloses an area")
}

fn world() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    world
}

#[test]
fn an_outline_has_to_enclose_an_area() {
    assert!(ZoneOutline::new(vec![]).is_none(), "no corners");
    assert!(
        ZoneOutline::new(vec![p(0.0, 0.0), p(1.0, 0.0)]).is_none(),
        "two corners"
    );
    assert!(
        ZoneOutline::new(vec![p(0.0, 0.0), p(5.0, 5.0), p(10.0, 10.0)]).is_none(),
        "three corners on one line"
    );
    assert!(
        ZoneOutline::new(vec![p(0.0, 0.0), p(5.0, 0.0), p(0.0, 0.0)]).is_none(),
        "a ring that goes out and comes back"
    );
}

#[test]
fn control_an_outline_that_encloses_an_area_is_made() {
    let outline = ZoneOutline::new(vec![p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0)]);
    assert!(outline.is_some(), "a triangle");
}

#[test]
fn a_closing_corner_that_names_the_first_again_is_dropped() {
    let open = vec![p(0.0, 0.0), p(10.0, 0.0), p(5.0, 8.0)];
    let mut closed = open.clone();
    closed.push(open[0]);
    let outline = ZoneOutline::new(closed).expect("a triangle");
    assert_eq!(outline.corners(), open.as_slice());
}

#[test]
fn the_corners_are_kept_in_the_order_given() {
    let outline = l_shape();
    assert_eq!(outline.corners().len(), 6);
    assert_eq!(outline.corners()[0], p(0.0, 0.0));
    assert_eq!(outline.corners()[3], p(5.0, 5.0));
}

#[test]
fn the_bounds_of_an_outline_is_the_box_round_its_corners() {
    assert_eq!(l_shape().bounds(), Rect::new(p(0.0, 0.0), p(10.0, 10.0)));
}

#[test]
fn a_zone_with_an_outline_is_not_among_the_zones() {
    let mut world = world();
    world.spawn_entity((Zone::keepout(Rect::new(p(20.0, 5.0), p(30.0, 15.0)), 0b11),));
    world.spawn_outlined_zone(
        Zone::keepout(Rect::new(p(0.0, 0.0), p(1.0, 1.0)), 0b01),
        l_shape(),
    );

    let rectangles = world.zones();
    assert_eq!(rectangles.len(), 1, "only the rectangle: {rectangles:?}");
    assert_eq!(
        rectangles[0].1.bounds,
        Rect::new(p(20.0, 5.0), p(30.0, 15.0))
    );
}

#[test]
fn the_zone_with_an_outline_is_among_the_outlined_zones_with_the_box_it_is_in() {
    let mut world = world();
    world.spawn_entity((Zone::keepout(Rect::new(p(20.0, 5.0), p(30.0, 15.0)), 0b11),));
    // The bounds the caller passes is not the one that is kept.
    world.spawn_outlined_zone(
        Zone::keepout(Rect::new(p(0.0, 0.0), p(1.0, 1.0)), 0b01),
        l_shape(),
    );

    let outlined = world.outlined_zones();
    assert_eq!(outlined.len(), 1);
    let (_, zone, outline) = &outlined[0];
    assert_eq!(zone.bounds, outline.bounds(), "the two never disagree");
    assert_eq!(zone.bounds, Rect::new(p(0.0, 0.0), p(10.0, 10.0)));
    assert_eq!(zone.layer_mask, 0b01);
}

#[test]
fn all_the_zones_hold_both_kinds_in_build_order() {
    let mut world = world();
    world.spawn_outlined_zone(
        Zone::keepout(Rect::default(), 0b01).with_name("first"),
        l_shape(),
    );
    world.spawn_entity((
        Zone::keepout(Rect::new(p(20.0, 5.0), p(30.0, 15.0)), 0b11).with_name("second"),
    ));

    let all = world.all_zones();
    let names: Vec<_> = all
        .iter()
        .map(|(_, z, _)| z.name.clone().unwrap())
        .collect();
    assert_eq!(names, ["first", "second"]);
    assert!(all[0].2.is_some() && all[1].2.is_none());
}

#[test]
fn a_pour_can_have_an_outline_and_keeps_its_net() {
    let mut world = world();
    let gnd = world.intern_net("GND");
    world.spawn_outlined_zone(
        Zone::copper_pour_for_net(Rect::default(), 0b10, gnd),
        l_shape(),
    );
    let outlined = world.outlined_zones();
    assert_eq!(outlined[0].1.kind, ZoneKind::CopperPour);
    assert_eq!(outlined[0].1.net, Some(gnd));
    assert!(
        world.zones().is_empty(),
        "and it is not a rectangle to a reader of zones()"
    );
}

#[test]
#[should_panic(expected = "only a keepout or a copper pour has an outline")]
fn a_flex_region_cannot_have_an_outline() {
    let mut world = world();
    world.spawn_outlined_zone(Zone::flex(Rect::default(), 0b11), l_shape());
}

#[test]
#[should_panic(expected = "only a keepout or a copper pour has an outline")]
fn a_named_area_cannot_have_an_outline() {
    let mut world = world();
    let region = Zone {
        kind: ZoneKind::Region,
        ..Zone::keepout(Rect::default(), 0b11)
    };
    world.spawn_outlined_zone(region, l_shape());
}

#[test]
fn control_a_rectangle_zone_is_among_the_zones_and_has_no_outline() {
    let mut world = world();
    world.spawn_entity((Zone::copper_pour(Rect::new(p(1.0, 1.0), p(9.0, 9.0)), 0b01),));
    assert_eq!(world.zones().len(), 1);
    assert!(world.outlined_zones().is_empty());
    assert!(world.all_zones()[0].2.is_none());
}

#[test]
fn the_outline_answers_whether_a_point_is_in_it() {
    let outline = l_shape();
    assert!(outline.contains(p(2.0, 2.0)));
    assert!(outline.contains(p(5.0, 0.0)), "on an edge");
    assert!(!outline.contains(p(8.0, 8.0)), "in the notch");
    assert!(outline.meets(Rect::new(p(4.0, 4.0), p(6.0, 6.0))));
    assert!(!outline.meets(Rect::new(p(7.0, 7.0), p(9.0, 9.0))));
}

#[test]
fn a_zone_the_language_has_no_word_for_is_not_written_as_its_box() {
    let mut world = world();
    let gnd = world.intern_net("GND");
    world.spawn_outlined_zone(
        Zone::copper_pour_for_net(Rect::default(), 0b01, gnd),
        l_shape(),
    );
    world.spawn_outlined_zone(Zone::keepout(Rect::default(), 0b01), l_shape());

    let written = cypcb_world::dsl::board_as_dsl_reporting(&mut world);

    assert!(
        !written.source.contains("zone"),
        "no pour:\n{}",
        written.source
    );
    assert!(
        !written.source.contains("keepout"),
        "no keepout:\n{}",
        written.source
    );
    assert_eq!(written.not_written.len(), 1, "{:?}", written.not_written);
    assert!(
        written.not_written[0].starts_with("2 zone(s) that are not rectangles not written"),
        "{:?}",
        written.not_written
    );
}

#[test]
fn control_a_rectangle_zone_is_written_and_nothing_is_said() {
    let mut world = world();
    world.spawn_entity((Zone::keepout(Rect::new(p(1.0, 1.0), p(9.0, 9.0)), 0b01),));

    let written = cypcb_world::dsl::board_as_dsl_reporting(&mut world);

    assert!(written.source.contains("keepout"), "{}", written.source);
    assert!(written.not_written.is_empty(), "{:?}", written.not_written);
}
