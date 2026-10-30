//! A trace on no net is a wall to every net the router routes.
//!
//! `cargo test -p cypcb-autoroute --test a_trace_on_no_net_is_a_wall_to_every_net`
//!
//! Copper a person drew on no net - KiCad's net 0 - was read as copper of the
//! first net the board interned. The router lets a net cross its own hand
//! copper and end on it, so a wall drawn on no net was a wall to every net but
//! that one, which went through it.
//!
//! J1 and J2 are on `SIG`, the first net. A trace on no net runs the whole
//! height of the board between them on the top layer. The only way across is
//! under it, through a via, and the via optimizer, which takes vias out
//! wherever a direct segment would do, has to see the wall too.

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::{Nm, Point, Rect};
use cypcb_router::{apply_routes, RoutingStatus};
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, Position, RefDes, Rotation, Value};

fn board(wall_net: Option<&str>) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);

    let mut library = FootprintLibrary::new();
    library.register(
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
        .with_courtyard_holding_its_pads(),
    );
    world.set_footprints(library.clone());

    let sig = world.intern_net("SIG");
    for (refdes, x) in [("J1", 10.0), ("J2", 30.0)] {
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), sig));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new(""),
            Position(Point::from_mm(x, 15.0)),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }

    let wall_net = wall_net.map(|name| world.intern_net(name));
    let mut wall = Trace::new(sig);
    wall.net_id = wall_net;
    wall.layer = Layer::TopCopper;
    wall.width = Nm::from_mm(0.25);
    wall.source = TraceSource::Manual;
    wall.add_segment(TraceSegment::new(
        Point::from_mm(20.0, 0.0),
        Point::from_mm(20.0, 30.0),
    ));
    match wall_net {
        Some(net) => world.ecs_mut().spawn((wall, net)),
        None => world.ecs_mut().spawn((wall,)),
    };

    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

/// How many segments the router drew on the top layer across x = 20mm, and
/// how many vias it placed.
fn crossings(wall_net: Option<&str>) -> (usize, usize) {
    let (mut world, library) = board(wall_net);
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "the connection is routed: {:?}",
        result.status
    );
    let wall_x = Nm::from_mm(20.0).0;
    let across = result
        .routes
        .iter()
        .filter(|segment| segment.layer == Layer::TopCopper)
        .filter(|segment| {
            let (a, b) = (segment.start.x.0, segment.end.x.0);
            a.min(b) <= wall_x && wall_x <= a.max(b)
        })
        .count();
    let vias = result.vias.len();
    apply_routes(&mut world, &result);
    (across, vias)
}

#[test]
fn a_wall_on_no_net_is_crossed_under_not_through() {
    let (across, vias) = crossings(None);
    assert_eq!(across, 0, "the route went through a trace on no net");
    assert!(vias >= 2, "the way across is under it: {vias} via(s)");
}

#[test]
fn a_wall_on_the_net_itself_is_the_control() {
    // The same wall on SIG is SIG's own copper: the route may cross it and
    // needs no via. This is what a wall on no net used to be.
    let (across, vias) = crossings(Some("SIG"));
    assert_eq!(vias, 0);
    assert!(across <= 1);
}

/// The smallest distance in mm from (20, 15) to any segment the router drew.
fn nearest_to_the_via(vias_on: Option<&str>) -> f64 {
    let (mut world, library) = board(None);
    // Take the wall away: the via alone is in the way, on the straight line
    // between the two pins.
    let wall = world
        .ecs_mut()
        .query::<bevy_ecs::entity::Entity>()
        .iter(world.ecs())
        .filter(|e| world.ecs().get::<Trace>(*e).is_some())
        .collect::<Vec<_>>();
    for entity in wall {
        world.ecs_mut().despawn(entity);
    }
    let net = vias_on.map(|name| world.intern_net(name));
    let mut via = cypcb_world::components::trace::Via::new(
        Point::from_mm(20.0, 15.0),
        world.intern_net("SIG"),
    );
    via.net_id = net;
    match net {
        Some(net) => world.ecs_mut().spawn((via, net)),
        None => world.ecs_mut().spawn((via,)),
    };
    world.rebuild_spatial_index_from_library(&library);

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "the connection is routed: {:?}",
        result.status
    );
    let (px, py) = (20.0, 15.0);
    result
        .routes
        .iter()
        .map(|segment| {
            let (ax, ay) = (segment.start.x.to_mm(), segment.start.y.to_mm());
            let (bx, by) = (segment.end.x.to_mm(), segment.end.y.to_mm());
            let (dx, dy) = (bx - ax, by - ay);
            let t = (((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy).max(1e-12))
                .clamp(0.0, 1.0);
            ((ax + t * dx - px).powi(2) + (ay + t * dy - py).powi(2)).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn a_via_on_no_net_is_routed_round_not_through() {
    // The via is 0.6mm across. A route that passes through its ring comes
    // within 0.3mm of its centre.
    assert!(nearest_to_the_via(None) > 0.3);
}

#[test]
fn a_via_on_the_net_itself_is_the_control() {
    // The same via on SIG is SIG's own copper: the route may run over it.
    assert!(nearest_to_the_via(Some("SIG")) <= 0.3);
}
