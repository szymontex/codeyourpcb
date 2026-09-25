//! A route to a pad its net has half-wired by hand ends on that wire.
//!
//! The pad and the hand trace it sits on are one conductor, but the search
//! only knew the pad: it ran from pad to pad and laid its own copper along the
//! hand trace the whole way. On a 20mm connection the designer had drawn
//! 16.5mm of, the router drew those 16.5mm a second time. The hand copper is
//! now where the route may leave from, when its pad is on the tree already,
//! and where it arrives, when its pad is the one being added.

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::Rect;
use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
use cypcb_router::{apply_routes, RoutingStatus};
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, Position, RefDes, Rotation, Value};

/// J1 at x=10mm and J2 at x=30mm on one net, and a hand trace of that net
/// from `hand_from` to `hand_to` along y=15mm.
fn board(hand_from: f64, hand_to: f64) -> (BoardWorld, FootprintLibrary) {
    board_with_j2_on(hand_from, hand_to, Layer::TopCopper, 30.0)
}

fn board_with_j2_on(
    hand_from: f64,
    hand_to: f64,
    j2_layer: Layer,
    j2_x: f64,
) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);

    let mut library = FootprintLibrary::new();
    for (name, layer) in [("PAD1", Layer::TopCopper), ("PAD2", j2_layer)] {
        library.register(Footprint {
            name: name.into(),
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
                layers: vec![layer],
                mask_margin: None,
            }],
        });
    }
    world.set_footprints(library.clone());

    let sig = world.intern_net("SIG");
    for (refdes, x, footprint) in [("J1", 10.0, "PAD1"), ("J2", j2_x, "PAD2")] {
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), sig));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new(""),
            Position(Point::from_mm(x, 15.0)),
            Rotation::ZERO,
            FootprintRef::new(footprint),
            connections,
        );
    }

    let mut trace = Trace::new(sig);
    trace.layer = Layer::TopCopper;
    trace.width = Nm::from_mm(0.25);
    trace.source = TraceSource::Manual;
    trace.add_segment(TraceSegment::new(
        Point::from_mm(hand_from, 15.0),
        Point::from_mm(hand_to, 15.0),
    ));
    world.ecs_mut().spawn((trace, sig));

    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

/// Route the board and return the copper the router drew, in mm, and the
/// net-split faults the checker finds on the result.
fn route(hand_from: f64, hand_to: f64) -> (f64, usize) {
    let (world, library) = board(hand_from, hand_to);
    let (drawn_mm, _, splits) = route_world(world, library);
    (drawn_mm, splits)
}

/// The copper drawn in mm, the vias placed and the net-split faults.
fn route_world(mut world: BoardWorld, library: FootprintLibrary) -> (f64, usize, usize) {
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "the connection is routed: {:?}",
        result.status
    );
    let drawn_mm: f64 = result
        .routes
        .iter()
        .map(|segment| {
            let dx = (segment.end.x.0 - segment.start.x.0) as f64;
            let dy = (segment.end.y.0 - segment.start.y.0) as f64;
            (dx * dx + dy * dy).sqrt() / 1e6
        })
        .sum();

    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);
    let splits = run_drc(&mut world, &DesignRules::jlcpcb_2layer())
        .violations
        .iter()
        .filter(|violation| violation.kind == ViolationKind::NetSplit)
        .count();
    (drawn_mm, result.vias.len(), splits)
}

#[test]
fn a_route_leaves_from_the_hand_trace_its_pad_sits_on() {
    // The hand trace runs from J1 to 3mm short of J2. What is left to draw is
    // those 3mm less the two half-pads, 2.5mm; the route used to be 19.05mm.
    let (drawn_mm, splits) = route(10.0, 27.0);
    assert!(
        drawn_mm < 4.0,
        "the route runs beside the hand trace: {drawn_mm:.3}mm"
    );
    assert_eq!(splits, 0, "the route ends on the hand trace, not beside it");
}

#[test]
fn a_route_arrives_on_the_hand_trace_its_pad_sits_on() {
    // The same gap from the other side: the hand trace runs from J2 to 3mm
    // short of J1, so the pad the route adds is the one on the hand trace.
    let (drawn_mm, splits) = route(13.0, 30.0);
    assert!(
        drawn_mm < 4.0,
        "the route runs beside the hand trace: {drawn_mm:.3}mm"
    );
    assert_eq!(splits, 0, "the route ends on the hand trace, not beside it");
}

#[test]
fn a_route_leaves_from_a_via_the_designer_placed_on_the_hand_trace() {
    // J2 is on the bottom and the top is kept out in front of it. The hand
    // trace ends in a via the designer placed, so the net is on the bottom
    // already at x=27mm: the route starts from the via's ring there and needs
    // no via of its own. Without the via's ring the search leaves from the
    // top trace and drills a second via beside the first.
    use cypcb_core::Rect as CoreRect;
    use cypcb_world::components::trace::Via;
    use cypcb_world::components::zone::{Zone, ZoneKind};

    let (mut world, library) = board_with_j2_on(10.0, 27.0, Layer::BottomCopper, 30.0);
    let sig = world.intern_net("SIG");
    world
        .ecs_mut()
        .spawn((Via::new(Point::from_mm(27.0, 15.0), sig), sig));
    world.spawn_entity(Zone {
        bounds: CoreRect {
            min: Point::from_mm(28.0, 0.0),
            max: Point::from_mm(40.0, 30.0),
        },
        kind: ZoneKind::Keepout,
        layer_mask: Layer::TopCopper.to_copper_mask(),
        name: None,
        net: None,
    });
    world.rebuild_spatial_index_from_library(&library);

    let (drawn_mm, vias, splits) = route_world(world, library);
    assert_eq!(
        vias, 0,
        "the placed via already takes the net to the bottom"
    );
    assert!(
        drawn_mm < 4.0,
        "the route runs beside the hand trace: {drawn_mm:.3}mm"
    );
    assert_eq!(splits, 0, "the route ends on the via, not beside it");
}

/// A pad of `net` with the one-pad footprint at `at`.
fn place(world: &mut BoardWorld, refdes: &str, at: (f64, f64), net: cypcb_world::NetId) {
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

/// A hand trace of `net` on top through `points`, `width` mm wide.
fn hand(world: &mut BoardWorld, net: cypcb_world::NetId, width: f64, points: &[(f64, f64)]) {
    let mut trace = Trace::new(net);
    trace.layer = Layer::TopCopper;
    trace.width = Nm::from_mm(width);
    trace.source = TraceSource::Manual;
    for pair in points.windows(2) {
        trace.add_segment(TraceSegment::new(
            Point::from_mm(pair[0].0, pair[0].1),
            Point::from_mm(pair[1].0, pair[1].1),
        ));
    }
    world.ecs_mut().spawn((trace, net));
}

#[test]
fn a_later_connection_leaves_from_hand_copper_the_tree_reached_earlier() {
    // J2 carries a hand trace that runs up and back left, ending 3mm above J3.
    // The spanning tree joins J1 to J2 first and then J3 from J1, its nearest
    // pad - but J2's wire is on the tree by then, and J3 is 2.5mm from it
    // against 9mm from J1.
    // The board's own hand trace is J2's first leg, straight up from it.
    let (mut world, library) = board_with_j2_on(18.0, 18.0, Layer::TopCopper, 18.0);
    let sig = world.intern_net("SIG");
    place(&mut world, "J3", (11.0, 24.0), sig);
    hand(
        &mut world,
        sig,
        0.25,
        &[(18.0, 15.0), (18.0, 27.0), (11.0, 27.0)],
    );
    world.rebuild_spatial_index_from_library(&library);

    let (drawn_mm, _, splits) = route_world(world, library);
    assert!(
        drawn_mm < 12.0,
        "J3 is routed from J1, not from the wire already on the tree: {drawn_mm:.3}mm"
    );
    assert_eq!(splits, 0, "every pad is on the net");
}

#[test]
fn a_wire_drawn_in_two_pieces_that_touch_is_one_wire() {
    // J1's wire is drawn as two traces, the second starting where the first
    // ends. They are one conductor, so the route to J2 leaves from the far
    // end of the second one.
    let (mut world, library) = board(10.0, 20.0);
    let sig = world.intern_net("SIG");
    hand(&mut world, sig, 0.25, &[(20.0, 15.0), (27.0, 15.0)]);
    world.rebuild_spatial_index_from_library(&library);

    let (drawn_mm, _, splits) = route_world(world, library);
    assert!(
        drawn_mm < 4.0,
        "the route runs beside the second trace: {drawn_mm:.3}mm"
    );
    assert_eq!(splits, 0, "the route ends on the wire, not beside it");
}

#[test]
fn a_net_wired_through_a_pad_two_traces_meet_on_is_not_routed_again() {
    // J1 to J2 by one trace, J2 to J3 by another: every pad is on copper, and
    // the two traces meet only on J2, 0.8mm apart across its copper. Nothing
    // is left to route.
    let (mut world, library) = board_with_j2_on(10.0, 19.6, Layer::TopCopper, 20.0);
    let sig = world.intern_net("SIG");
    place(&mut world, "J3", (30.0, 15.0), sig);
    hand(&mut world, sig, 0.25, &[(20.4, 15.0), (30.0, 15.0)]);
    world.rebuild_spatial_index_from_library(&library);

    let (drawn_mm, _, splits) = route_world(world, library);
    assert_eq!(drawn_mm, 0.0, "a connection the hand copper already makes");
    assert_eq!(splits, 0, "every pad is on the net");
}

#[test]
fn two_traces_that_cross_are_one_wire() {
    // A second trace crosses J1's wire in an X, no end of either near the
    // other, and ends beside J2. The crossing joins them, so the route to J2
    // leaves from that end - 3.618mm drawn - rather than from the end of J1's
    // wire, 5.080mm.
    let (mut world, library) = board(10.0, 25.0);
    let sig = world.intern_net("SIG");
    hand(&mut world, sig, 0.25, &[(12.0, 20.0), (28.0, 13.0)]);
    world.rebuild_spatial_index_from_library(&library);

    let (drawn_mm, _, splits) = route_world(world, library);
    assert!(
        drawn_mm < 4.5,
        "the route leaves from the end of J1's wire: {drawn_mm:.3}mm"
    );
    assert_eq!(splits, 0, "the route ends on the wire, not beside it");
}

#[test]
fn a_route_arrives_on_the_nearest_part_of_the_hand_copper() {
    // J2's wire goes up and back left, passing 7mm above J1. Aimed at J2, the
    // search reaches the wire where it leaves J2, 20mm away; the part above
    // J1 is a third of that.
    let (mut world, library) = board(30.0, 30.0);
    let sig = world.intern_net("SIG");
    hand(
        &mut world,
        sig,
        0.25,
        &[(30.0, 15.0), (30.0, 22.0), (5.0, 22.0)],
    );
    world.rebuild_spatial_index_from_library(&library);

    let (drawn_mm, _, splits) = route_world(world, library);
    assert!(
        drawn_mm < 10.0,
        "the route runs to J2's end of the wire: {drawn_mm:.3}mm"
    );
    assert_eq!(splits, 0, "the route ends on the wire, not beside it");
}
