//! A trace across a plane cuts it, and half of it can end up connected to
//! nothing.
//!
//! This is the case the rule exists for, built as a board rather than as a
//! list of rectangles: a ground pour, one ground pad at the bottom of it, and
//! a signal trace running the full width above that pad. The copper above the
//! trace is a sheet of ground with no way to reach ground.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
use cypcb_world::components::trace::{Trace, TraceSegment};
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, PadDef};
use cypcb_world::{BoardWorld, Position, RefDes, Rotation, Value};

/// A board with a ground pour, and a ground pad wherever the caller wants it.
fn board_with_pour(pad_at: Point, cut: bool) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);

    let gnd = world.intern_net("GND");
    let sig = world.intern_net("SIG");

    let mut library = cypcb_world::footprint::FootprintLibrary::new();
    library.register(Footprint {
        name: "PAD1".into(),
        description: String::new(),
        bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
        courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".into(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            drill: None,
            layers: vec![Layer::TopCopper],
        }],
    });
    world.set_footprints(library);

    let mut connections = NetConnections::new();
    connections.add(PinConnection::new("1".to_string(), gnd));
    world.spawn_component(
        RefDes::new("J1"),
        Value::new(""),
        Position(pad_at),
        Rotation::ZERO,
        FootprintRef::new("PAD1"),
        connections,
    );

    if cut {
        // A signal trace running past both edges of the pour, so what it
        // leaves behind is two separate sheets rather than one ring.
        let mut trace = Trace::new(sig);
        trace.layer = Layer::TopCopper;
        trace.width = Nm::from_mm(0.2);
        trace.add_segment(TraceSegment::new(
            Point::from_mm(2.0, 20.0),
            Point::from_mm(38.0, 20.0),
        ));
        world.ecs_mut().spawn((trace, sig));
    }

    world.spawn_entity(Zone {
        bounds: Rect {
            min: Point::from_mm(5.0, 5.0),
            max: Point::from_mm(35.0, 35.0),
        },
        kind: ZoneKind::CopperPour,
        layer_mask: Layer::TopCopper.to_copper_mask(),
        name: Some("gnd".to_string()),
        net: Some(gnd),
    });

    world.rebuild_spatial_index_from_library(&world.footprints().clone());
    world
}

fn islands(world: &mut BoardWorld) -> usize {
    run_drc(world, &DesignRules::jlcpcb_2layer())
        .violations
        .iter()
        .filter(|violation| violation.kind == ViolationKind::PourIsland)
        .count()
}

#[test]
fn copper_left_on_the_far_side_of_a_cut_is_reported() {
    // Pad low, trace across the middle: everything above the trace is ground
    // copper that cannot reach ground.
    let mut world = board_with_pour(Point::from_mm(20.0, 10.0), true);
    assert_eq!(
        islands(&mut world),
        1,
        "half a ground plane with no way to reach ground is one island"
    );
}

#[test]
fn an_uncut_plane_with_a_pad_on_it_says_nothing() {
    let mut world = board_with_pour(Point::from_mm(20.0, 10.0), false);
    assert_eq!(
        islands(&mut world),
        0,
        "a whole plane bridged to its own pad is not an island"
    );
}

#[test]
fn a_plane_whose_net_has_no_pad_under_it_is_reported_once() {
    // The pad sits outside the pour, so nothing bridges to the plane at all.
    // One report, not one per rectangle the fill produced.
    let mut world = board_with_pour(Point::from_mm(38.0, 38.0), false);
    assert_eq!(islands(&mut world), 1);
}
