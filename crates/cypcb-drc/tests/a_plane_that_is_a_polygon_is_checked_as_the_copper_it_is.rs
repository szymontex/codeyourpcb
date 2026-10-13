//! A plane drawn as a polygon is checked as the copper it is.
//!
//! `cargo test -p cypcb-drc --test a_plane_that_is_a_polygon_is_checked_as_the_copper_it_is`
//!
//! `a_plane_cut_in_two_is_reported` builds a rectangular plane, a pad and a
//! trace across it. This is the same board with the plane a ring: the fill the
//! rules measure is cut from the ring, so a trace across it strands the part
//! beyond, and a pad in the notch of an L is on no copper of the plane although
//! it stands inside the L's bounding box. A rectangle drawn as the same
//! board is the control.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
use cypcb_world::components::trace::{Trace, TraceSegment};
use cypcb_world::components::zone::{Zone, ZoneOutline};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, PadDef};
use cypcb_world::{BoardWorld, Position, RefDes, Rotation, Value};

fn p(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn pad_footprint() -> Footprint {
    Footprint {
        placement_origin: None,
        name: "PAD1".into(),
        description: String::new(),
        bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
        courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
        silk: Vec::new(),
        pads: vec![PadDef {
            hole_offset: None,
            number: "1".into(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        }],
    }
    .with_courtyard_holding_its_pads()
}

/// A ground pour as `ring` - or as its bounding rectangle - with a ground pad at
/// each of `pads`, and a signal trace across the board at y = 20 when `cut`.
fn board(ring: Option<ZoneOutline>, pads: &[Point], cut: bool) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
    let gnd = world.intern_net("GND");
    let sig = world.intern_net("SIG");

    let mut library = cypcb_world::footprint::FootprintLibrary::new();
    library.register(pad_footprint());
    world.set_footprints(library);

    for (i, at) in pads.iter().enumerate() {
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), gnd));
        world.spawn_component(
            RefDes::new(format!("J{}", i + 1)),
            Value::new(""),
            Position(*at),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }

    if cut {
        let mut trace = Trace::new(sig);
        trace.layer = Layer::TopCopper;
        trace.width = Nm::from_mm(0.2);
        trace.add_segment(TraceSegment::new(p(2.0, 20.0), p(38.0, 20.0)));
        world.ecs_mut().spawn((trace, sig));
    }

    let mask = Layer::TopCopper.to_copper_mask();
    match ring {
        Some(ring) => {
            world.spawn_outlined_zone(Zone::copper_pour_for_net(Rect::default(), mask, gnd), ring);
        }
        None => {
            world.spawn_entity((Zone::copper_pour_for_net(
                Rect::new(p(5.0, 5.0), p(35.0, 35.0)),
                mask,
                gnd,
            ),));
        }
    }
    world.rebuild_spatial_index_from_library(&world.footprints().clone());
    world
}

/// A pentagon with no edge on an axis, inside 5..35.
fn slanted() -> ZoneOutline {
    ZoneOutline::new(vec![
        p(5.0, 5.0),
        p(35.0, 6.0),
        p(34.0, 35.0),
        p(20.0, 36.0),
        p(6.0, 34.0),
    ])
    .expect("a pentagon")
}

/// An L: 20 x 20 with the upper right quarter cut away, from (5, 5).
fn l_ring() -> ZoneOutline {
    ZoneOutline::new(vec![
        p(5.0, 5.0),
        p(25.0, 5.0),
        p(25.0, 15.0),
        p(15.0, 15.0),
        p(15.0, 25.0),
        p(5.0, 25.0),
    ])
    .expect("an L")
}

fn count(world: &mut BoardWorld, kind: ViolationKind) -> usize {
    run_drc(world, &DesignRules::jlcpcb_2layer())
        .violations
        .iter()
        .filter(|violation| violation.kind == kind)
        .count()
}

#[test]
fn copper_on_the_far_side_of_a_cut_of_a_ring_is_an_island() {
    let mut world = board(Some(slanted()), &[p(20.0, 10.0)], true);
    assert_eq!(count(&mut world, ViolationKind::PourIsland), 1);
}

#[test]
fn an_uncut_ring_with_a_pad_on_it_says_nothing() {
    let mut world = board(Some(slanted()), &[p(20.0, 10.0)], false);
    assert_eq!(count(&mut world, ViolationKind::PourIsland), 0);
}

#[test]
fn a_ring_with_no_pad_of_its_net_under_it_is_reported_once() {
    let mut world = board(Some(slanted()), &[p(38.0, 38.0)], false);
    assert_eq!(
        count(&mut world, ViolationKind::PourIsland),
        1,
        "one report, not one per piece"
    );
}

#[test]
fn the_island_is_the_half_above_the_cut() {
    let mut world = board(Some(slanted()), &[p(20.0, 10.0)], true);
    let report = run_drc(&mut world, &DesignRules::jlcpcb_2layer());
    let island = report
        .violations
        .iter()
        .find(|v| v.kind == ViolationKind::PourIsland)
        .expect("the island is reported");
    let area = island.area.expect("an island is an area");
    assert!(area.min.y.0 > Nm::from_mm(20.0).0, "got {area:?}");
}

#[test]
fn a_pad_in_the_notch_of_an_l_is_on_no_copper_of_the_plane() {
    // Both pads are inside the L's bounding box; one is in the L and one is in
    // the notch.
    let mut world = board(Some(l_ring()), &[p(10.0, 10.0), p(20.0, 20.0)], false);
    let kinds: Vec<_> = run_drc(&mut world, &DesignRules::jlcpcb_2layer())
        .violations
        .iter()
        .map(|v| v.kind.to_string())
        .collect();
    assert_eq!(
        count(&mut world, ViolationKind::UnroutedPin),
        1,
        "{kinds:?}"
    );
    // A pin nothing reaches is reported by one rule and not by both.
    assert_eq!(count(&mut world, ViolationKind::NetSplit), 0, "{kinds:?}");
}

#[test]
fn control_two_pads_on_the_plane_are_both_reached() {
    let mut world = board(Some(l_ring()), &[p(10.0, 10.0), p(20.0, 10.0)], false);
    assert_eq!(count(&mut world, ViolationKind::UnroutedPin), 0);
    assert_eq!(count(&mut world, ViolationKind::NetSplit), 0);
}

#[test]
fn control_the_same_two_pads_on_a_rectangle_are_both_reached() {
    // The notch of the L is plane when the pour is the box.
    let mut world = board(None, &[p(10.0, 10.0), p(20.0, 20.0)], false);
    assert_eq!(count(&mut world, ViolationKind::UnroutedPin), 0);
    assert_eq!(count(&mut world, ViolationKind::NetSplit), 0);
}

/// A ground stub - a pad with a short ground trace - at (20, 20), where the
/// notch of the L is.
fn with_a_stub(mut world: BoardWorld) -> BoardWorld {
    let gnd = world.intern_net("GND");
    let mut trace = Trace::new(gnd);
    trace.layer = Layer::TopCopper;
    trace.width = Nm::from_mm(0.2);
    trace.add_segment(TraceSegment::new(p(20.0, 20.0), p(21.5, 20.0)));
    world.ecs_mut().spawn((trace, gnd));
    world
}

#[test]
fn a_stub_in_the_notch_is_a_second_piece_of_the_net() {
    let mut world = with_a_stub(board(
        Some(l_ring()),
        &[p(10.0, 10.0), p(20.0, 20.0)],
        false,
    ));
    assert_eq!(
        count(&mut world, ViolationKind::NetSplit),
        1,
        "copper in the notch is not the plane"
    );
}

#[test]
fn control_the_same_stub_on_a_rectangle_is_joined_to_the_plane() {
    let mut world = with_a_stub(board(None, &[p(10.0, 10.0), p(20.0, 20.0)], false));
    assert_eq!(count(&mut world, ViolationKind::NetSplit), 0);
}
