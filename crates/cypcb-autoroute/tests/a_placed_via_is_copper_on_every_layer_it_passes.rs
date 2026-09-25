//! A via already on the board is copper on every layer its hole passes.
//!
//! The grid marked traces only, so a via the designer placed - in a `.cypcb`
//! trace, from a KiCad import, or by a stitched pour - was invisible to every
//! route, and the shortest path ran straight across its ring. None of the six
//! benchmark boards and none of the examples that route carries such a via,
//! so these boards are the only place the fault shows.

use cypcb_autoroute::grid::{layer_to_index, RoutingGrid};
use cypcb_autoroute::via_optimizer::{optimize_vias, BoardObstacles};
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::{Nm, Point, Rect};
use cypcb_router::types::{RouteSegment, RoutingStatus, ViaPlacement};
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, NetId, Position, RefDes, Rotation, Value};

/// Where the placed via stands: on the straight line between the two pads.
const VIA_AT: (f64, f64) = (15.0, 10.0);

/// A four-layer board with a one-pad footprint on `layers`, and a pad of
/// `net` at each of `pins`.
fn board(
    layers: Vec<Layer>,
    drill: Option<Nm>,
    pins: &[(&str, (f64, f64), &str)],
) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 4);

    let mut library = FootprintLibrary::new();
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
            drill,
            slot: None,
            layers,
            mask_margin: None,
        }],
    });
    world.set_footprints(library.clone());

    for (refdes, at, net) in pins {
        let net = world.intern_net(net);
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), net));
        world.spawn_component(
            RefDes::new(*refdes),
            Value::new(""),
            Position(Point::from_mm(at.0, at.1)),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }
    (world, library)
}

/// A via from the top down to the second inner layer: its hole passes Top,
/// Inner(0) and Inner(1), and stops short of Bottom.
fn place_via(world: &mut BoardWorld, net: NetId) {
    let mut via = Via::new(Point::from_mm(VIA_AT.0, VIA_AT.1), net);
    via.end_layer = Layer::Inner(1);
    world.ecs_mut().spawn((via, net));
}

fn route(world: &mut BoardWorld, library: &FootprintLibrary) -> cypcb_router::types::RoutingResult {
    world.rebuild_spatial_index_from_library(library);
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    route_board(world, library, &rules, &AutorouteConfig::default())
}

/// How far a segment's copper edge is from the via's copper edge; below zero
/// the two overlap.
fn gap_to_via(segment: &RouteSegment, via: &Via) -> f64 {
    let (px, py) = (via.position.x.0 as f64, via.position.y.0 as f64);
    let (ax, ay) = (segment.start.x.0 as f64, segment.start.y.0 as f64);
    let (bx, by) = (segment.end.x.0 as f64, segment.end.y.0 as f64);
    let (dx, dy) = (bx - ax, by - ay);
    let length_sq = dx * dx + dy * dy;
    let t = if length_sq == 0.0 {
        0.0
    } else {
        (((px - ax) * dx + (py - ay) * dy) / length_sq).clamp(0.0, 1.0)
    };
    let centre = (px - ax - t * dx).hypot(py - ay - t * dy);
    (centre - segment.width.0 as f64 / 2.0 - via.outer_diameter.0 as f64 / 2.0) / 1e6
}

fn the_via(world: &mut BoardWorld) -> Via {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Via>();
    let vias: Vec<Via> = query.iter(ecs).copied().collect();
    assert_eq!(vias.len(), 1, "one via on the board before routing");
    vias[0]
}

#[test]
fn a_route_goes_around_a_via_another_net_placed() {
    // Two top pads with a via of another net on the straight line between
    // them. The shortest route runs through the via's ring; the grid has to
    // know the ring is there for the route to leave it alone.
    let (mut world, library) = board(
        vec![Layer::TopCopper],
        None,
        &[("J1", (5.0, 10.0), "CROSS"), ("J2", (25.0, 10.0), "CROSS")],
    );
    let other = world.intern_net("OTHER");
    place_via(&mut world, other);
    let via = the_via(&mut world);

    let result = route(&mut world, &library);
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "{:?}",
        result.status
    );

    let through: Vec<(Layer, f64)> = result
        .routes
        .iter()
        .filter(|segment| segment.layer.to_copper_mask() & via.copper_mask() != 0)
        .map(|segment| (segment.layer, gap_to_via(segment, &via)))
        .filter(|(_, gap)| *gap < 0.0)
        .collect();
    assert!(
        through.is_empty(),
        "route copper on the via's ring, on a layer its hole passes (layer, gap mm): {through:?}"
    );
}

#[test]
fn a_via_leaves_the_layers_its_hole_does_not_reach_open() {
    // The same via, and a net on the bottom: the hole stops at Inner(1), so the
    // bottom under it is free board and the straight route may cross it.
    let (mut world, library) = board(
        vec![Layer::BottomCopper],
        None,
        &[("J1", (5.0, 10.0), "UNDER"), ("J2", (25.0, 10.0), "UNDER")],
    );
    let other = world.intern_net("OTHER");
    place_via(&mut world, other);
    let via = the_via(&mut world);

    let result = route(&mut world, &library);
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "{:?}",
        result.status
    );

    let under = result
        .routes
        .iter()
        .filter(|segment| segment.layer == Layer::BottomCopper)
        .any(|segment| gap_to_via(segment, &via) < 0.0);
    assert!(
        under,
        "the bottom is outside the hole, so the straight route passes under the via"
    );
}

/// A three-pin net on a four-layer board, with or without the hand wiring
/// that joins J1 to J2 through a via; returns the routed length in mm.
fn three_pins(hand_wired: bool) -> (RoutingStatus, f64) {
    let every_layer = vec![
        Layer::TopCopper,
        Layer::BottomCopper,
        Layer::Inner(0),
        Layer::Inner(1),
    ];
    let (mut world, library) = board(
        every_layer,
        Some(Nm::from_mm(0.3)),
        &[
            ("J1", (5.0, 10.0), "SIG"),
            ("J2", (25.0, 10.0), "SIG"),
            ("J3", (15.0, 17.0), "SIG"),
        ],
    );
    if hand_wired {
        let net = world.intern_net("SIG");
        for (layer, from, to) in [
            (Layer::TopCopper, (5.0, 10.0), VIA_AT),
            (Layer::Inner(1), VIA_AT, (25.0, 10.0)),
        ] {
            let mut trace = Trace::new(net);
            trace.layer = layer;
            trace.width = Nm::from_mm(0.2);
            trace.source = TraceSource::Manual;
            trace.add_segment(TraceSegment::new(
                Point::from_mm(from.0, from.1),
                Point::from_mm(to.0, to.1),
            ));
            world.ecs_mut().spawn((trace, net));
        }
        place_via(&mut world, net);
    }

    let result = route(&mut world, &library);
    let length = result
        .routes
        .iter()
        .map(|segment| {
            ((segment.end.x.0 - segment.start.x.0) as f64)
                .hypot((segment.end.y.0 - segment.start.y.0) as f64)
        })
        .sum::<f64>()
        / 1e6;
    (result.status, length)
}

#[test]
fn a_net_its_own_via_already_joins_is_routed_only_to_the_pin_left_over() {
    // J1 and J2 are wired by hand: a top trace, a via down to Inner(1), and an
    // inner trace on. J3 is the net's third pin and still needs its route. The
    // via's net reaches the via's copper the way it reaches a hand trace's -
    // through the pads that copper already joins - so the net comes back
    // complete, and with one connection to draw instead of two.
    let (bare_status, bare) = three_pins(false);
    let (wired_status, wired) = three_pins(true);
    assert!(
        matches!(bare_status, RoutingStatus::Complete),
        "{bare_status:?}"
    );
    assert!(
        matches!(wired_status, RoutingStatus::Complete),
        "{wired_status:?}"
    );
    assert!(
        wired > 0.0 && wired < bare * 0.75,
        "J3 alone is routed on the wired board: {wired:.3} mm against {bare:.3} mm for all three pins"
    );
}

#[test]
fn the_grid_marks_a_placed_via_on_every_layer_its_hole_passes() {
    // Top, Inner(0) and Inner(1) are copper of the via; Bottom is not. The
    // middle layer is the one a via's two ends do not name.
    let (mut world, library) = board(vec![Layer::TopCopper], None, &[]);
    let other = world.intern_net("OTHER");
    place_via(&mut world, other);
    world.rebuild_spatial_index_from_library(&library);

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let grid = RoutingGrid::from_board(&mut world, &library, &rules, 63_500).expect("a board");
    let (x, y) = grid.nm_to_grid(Point::from_mm(VIA_AT.0, VIA_AT.1));

    let blocked: Vec<(Layer, bool)> = [
        Layer::TopCopper,
        Layer::Inner(0),
        Layer::Inner(1),
        Layer::BottomCopper,
    ]
    .into_iter()
    .map(|layer| {
        let index = layer_to_index(layer).expect("a copper layer");
        (layer, !grid.is_free(x, y, index))
    })
    .collect();
    assert_eq!(
        blocked,
        vec![
            (Layer::TopCopper, true),
            (Layer::Inner(0), true),
            (Layer::Inner(1), true),
            (Layer::BottomCopper, false),
        ]
    );
}

/// How many of a via pair the optimizer keeps, when the direct segment it
/// would put back runs along Inner(0) past the placed via, or past nothing.
fn vias_kept_on_the_middle_layer(via_placed: bool) -> usize {
    let (mut world, library) = board(vec![Layer::TopCopper], None, &[]);
    let net = world.intern_net("SIG");
    if via_placed {
        let other = world.intern_net("OTHER");
        place_via(&mut world, other);
    }
    let board = BoardObstacles::from_board(&mut world, &library);

    // On Inner(0), down to Bottom to get past the via, and back up.
    let segment = |layer, from: f64, to: f64| {
        RouteSegment::new(
            net,
            layer,
            Nm::from_mm(0.2),
            Point::from_mm(from, VIA_AT.1),
            Point::from_mm(to, VIA_AT.1),
        )
    };
    let segments = vec![
        segment(Layer::Inner(0), 5.0, 10.0),
        segment(Layer::BottomCopper, 10.0, 20.0),
        segment(Layer::Inner(0), 20.0, 25.0),
    ];
    let vias = vec![
        ViaPlacement::new(
            net,
            Point::from_mm(10.0, VIA_AT.1),
            Nm::from_mm(0.3),
            Layer::Inner(0),
            Layer::BottomCopper,
        ),
        ViaPlacement::new(
            net,
            Point::from_mm(20.0, VIA_AT.1),
            Nm::from_mm(0.3),
            Layer::BottomCopper,
            Layer::Inner(0),
        ),
    ];

    let (_, kept) = optimize_vias(segments, vias, &board, Nm::from_mm(0.127), Nm(0));
    kept.len()
}

#[test]
fn the_via_optimizer_keeps_a_pair_that_gets_past_a_placed_via() {
    // Without the placed via the pair has no reason to exist and goes: that is
    // the control that shows the direct segment is otherwise clear.
    assert_eq!(
        vias_kept_on_the_middle_layer(false),
        0,
        "nothing in the way"
    );
    assert_eq!(
        vias_kept_on_the_middle_layer(true),
        2,
        "the direct segment on Inner(0) would run through the placed via's ring"
    );
}
