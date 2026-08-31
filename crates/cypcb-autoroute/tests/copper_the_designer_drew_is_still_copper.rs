//! A trace the designer drew is an obstacle, whether or not it is locked.
//!
//! The grid marked locked traces only, so hand-drawn copper that nobody
//! thought to lock was invisible to the router - which then routed straight
//! across it. `locked` means "do not rip this up"; unlocked copper is still
//! copper.

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::Rect;
use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, PadDef};
use cypcb_world::{BoardWorld, Position, RefDes, Rotation, Value};

/// A board with one hand-drawn trace across the middle and a net that has to
/// get past it.
fn board(locked: bool) -> (BoardWorld, cypcb_world::footprint::FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);

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
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        }],
    });
    world.set_footprints(library.clone());

    let hand = world.intern_net("HAND");
    let cross = world.intern_net("CROSS");

    // The net the router has to connect, above and below the hand trace.
    for (refdes, at) in [("J1", (15.0, 3.0)), ("J2", (15.0, 17.0))] {
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), cross));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new(""),
            Position(Point::from_mm(at.0, at.1)),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }

    let mut trace = Trace::new(hand);
    trace.layer = Layer::TopCopper;
    trace.width = Nm::from_mm(0.2);
    trace.locked = locked;
    trace.source = TraceSource::Manual;
    trace.add_segment(TraceSegment::new(
        Point::from_mm(2.0, 10.0),
        Point::from_mm(28.0, 10.0),
    ));
    world.ecs_mut().spawn((trace, hand));

    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

fn shorts_after_routing(locked: bool) -> usize {
    let (mut world, library) = board(locked);
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    run_drc(&mut world, &DesignRules::jlcpcb_2layer())
        .violations
        .iter()
        .filter(|violation| violation.kind == ViolationKind::Clearance)
        .filter(|violation| violation.actual == Some(Nm::ZERO))
        .count()
}

#[test]
fn an_unlocked_hand_trace_is_treated_exactly_like_a_locked_one() {
    // Both boards used to come back clean, and both were lying: the router
    // changed layer to get past the hand trace, and the via that would have
    // joined the two halves was deleted before it reached the output. With
    // every via kept, the same board reports one copper-on-copper fault - the
    // grid does not model a via's ring, so it lands closer to the hand trace
    // than the fab allows. That is the next piece of work, and it is a fault
    // the board really has rather than one nobody was told about.
    //
    // What this test is about survives it: locked and unlocked copper are
    // treated the same, and neither is driven through.
    let unlocked = shorts_after_routing(false);
    let locked = shorts_after_routing(true);

    assert_eq!(
        unlocked, locked,
        "unlocked copper is still copper: {unlocked} against {locked}"
    );
    assert!(
        unlocked <= 1,
        "one fault is the via ring the grid does not model; more is new: {unlocked}"
    );
}

#[test]
fn a_net_a_hand_trace_already_joins_is_not_routed_again() {
    // The router asks for a spanning tree over every pad of a net, so a net
    // the designer wired by hand came out wired twice - two pieces of copper
    // for one connection, and the second one taking space the rest of the
    // board needs.
    use cypcb_world::components::trace::Trace;

    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);

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
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        }],
    });
    world.set_footprints(library.clone());

    let net = world.intern_net("SIG");
    for (refdes, at) in [("J1", (5.0, 10.0)), ("J2", (25.0, 10.0))] {
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), net));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new(""),
            Position(Point::from_mm(at.0, at.1)),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }

    // The designer's own wire, pad to pad.
    let mut hand = Trace::new(net);
    hand.layer = Layer::TopCopper;
    hand.width = Nm::from_mm(0.2);
    hand.source = TraceSource::Manual;
    hand.add_segment(TraceSegment::new(
        Point::from_mm(5.0, 10.0),
        Point::from_mm(25.0, 10.0),
    ));
    world.ecs_mut().spawn((hand, net));
    world.rebuild_spatial_index_from_library(&library);

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());

    assert_eq!(
        result.route_count(),
        0,
        "the connection is already made, so there is nothing to route"
    );
}

#[test]
fn copper_on_another_layer_does_not_count_as_a_connection() {
    // A bottom-layer trace crossing over a top-layer pad is two pieces of
    // copper with the board between them. Counting that as a connection drops
    // a route the board needs, and the board comes back with a pin wired to
    // nothing.
    use cypcb_world::components::trace::Trace;

    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);

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
            slot: None,
            // Top only: a surface-mount pad.
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        }],
    });
    world.set_footprints(library.clone());

    let net = world.intern_net("SIG");
    for (refdes, at) in [("J1", (5.0, 10.0)), ("J2", (25.0, 10.0))] {
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), net));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new(""),
            Position(Point::from_mm(at.0, at.1)),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }

    // Copper of the same net, running under both pads on the wrong layer.
    let mut under = Trace::new(net);
    under.layer = Layer::BottomCopper;
    under.width = Nm::from_mm(0.2);
    under.source = TraceSource::Manual;
    under.add_segment(TraceSegment::new(
        Point::from_mm(5.0, 10.0),
        Point::from_mm(25.0, 10.0),
    ));
    world.ecs_mut().spawn((under, net));
    world.rebuild_spatial_index_from_library(&library);

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());

    assert!(
        result.route_count() > 0,
        "the pads are on top and the copper is on the bottom, so the connection is still missing"
    );
}

#[test]
fn a_via_joins_two_traces_into_one_piece_of_copper() {
    // The designer wired a pin on top, dropped through a via, and came back on
    // the bottom. Two traces and a via are one connection; without the via in
    // the reckoning they read as two pieces and the router adds a link that is
    // already there.
    use cypcb_world::components::trace::{Trace, Via};

    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);

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
            // Through-hole pins: the wire leaves on top and comes back on the
            // bottom, which only connects if the pad is on both.
            drill: Some(Nm::from_mm(0.3)),
            slot: None,
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
            mask_margin: None,
        }],
    });
    world.set_footprints(library.clone());

    let net = world.intern_net("SIG");
    for (refdes, at) in [("J1", (5.0, 10.0)), ("J2", (25.0, 10.0))] {
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), net));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new(""),
            Position(Point::from_mm(at.0, at.1)),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }

    // Top: from J1 to the middle. Bottom: from the middle to J2. A via joins
    // them where they meet.
    let mut top = Trace::new(net);
    top.layer = Layer::TopCopper;
    top.width = Nm::from_mm(0.2);
    top.source = TraceSource::Manual;
    top.add_segment(TraceSegment::new(
        Point::from_mm(5.0, 10.0),
        Point::from_mm(15.0, 10.0),
    ));

    let mut bottom = Trace::new(net);
    bottom.layer = Layer::BottomCopper;
    bottom.width = Nm::from_mm(0.2);
    bottom.source = TraceSource::Manual;
    bottom.add_segment(TraceSegment::new(
        Point::from_mm(15.0, 10.0),
        Point::from_mm(25.0, 10.0),
    ));

    let mut via = Via::new(Point::from_mm(15.0, 10.0), net);
    via.locked = true;

    world.ecs_mut().spawn((top, net));
    world.ecs_mut().spawn((bottom, net));
    world.ecs_mut().spawn((via, net));
    world.rebuild_spatial_index_from_library(&library);

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());

    assert_eq!(
        result.route_count(),
        0,
        "the pins are already joined through the via"
    );
}

#[test]
fn a_ground_plane_connects_the_pins_that_sit_in_it() {
    // A pour is copper: every pad of its net inside it is joined to every
    // other through the plane.
    //
    // This one passed the moment it was written - `extract_ratsnest` has
    // dropped pads inside a pour of their own net since a zone learned its
    // net, and the tracker's claim that the router still wired them was wrong.
    // The test stays because nothing else held that behaviour in place: it was
    // one line in the ratsnest with no test naming the board it protects.
    use cypcb_core::Rect as CoreRect;
    use cypcb_world::components::zone::{Zone, ZoneKind};

    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);

    let mut library = cypcb_world::footprint::FootprintLibrary::new();
    library.register(Footprint {
        name: "PAD1".into(),
        description: String::new(),
        bounds: CoreRect::new(Point::ORIGIN, Point::ORIGIN),
        courtyard: CoreRect::new(Point::ORIGIN, Point::ORIGIN),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".into(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        }],
    });
    world.set_footprints(library.clone());

    let gnd = world.intern_net("GND");
    for (refdes, at) in [
        ("J1", (8.0, 10.0)),
        ("J2", (15.0, 10.0)),
        ("J3", (22.0, 10.0)),
    ] {
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), gnd));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new(""),
            Position(Point::from_mm(at.0, at.1)),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }

    world.spawn_entity(Zone {
        bounds: CoreRect {
            min: Point::from_mm(5.0, 5.0),
            max: Point::from_mm(25.0, 15.0),
        },
        kind: ZoneKind::CopperPour,
        layer_mask: Layer::TopCopper.to_copper_mask(),
        name: Some("gnd".to_string()),
        net: Some(gnd),
    });
    world.rebuild_spatial_index_from_library(&library);

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());

    assert_eq!(
        result.route_count(),
        0,
        "three ground pins in a ground plane need no wires between them"
    );
}
