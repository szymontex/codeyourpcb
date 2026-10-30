//! The router keeps off the edge of the board.
//!
//! `cargo test -p cypcb-autoroute --test the_router_keeps_off_the_edge_of_the_board`
//!
//! The grid was built from the size of the board alone, so a track ran through
//! a cutout, across the notch of a shaped outline and along the rim at no
//! distance from it. The edge is an obstacle the way a keepout is: the cells
//! off the board and the cells nearer an edge than the fab allows are shut to
//! tracks, and the cells nearer than a via's ring is wide are shut to vias.
//!
//! The numbers below are for the JLCPCB table at a 0.1mm grid, whose cell
//! centres lie at 0.05, 0.15, 0.25 ... from the edge of the board: an edge
//! clearance of 0.3mm and a 0.127mm track keep a track's centre 0.3635mm off
//! the edge, which shuts the cells centred at 0.05 to 0.35; a via drilled
//! 0.3mm has a ring 0.3mm wide on a side, so its centre keeps 0.6mm off the
//! edge, which shuts the cells centred at 0.05 to 0.55.

use cypcb_autoroute::edge_guard::EdgeGuard;
use cypcb_autoroute::grid::{RoutingGrid, CELL_ZONE};
use cypcb_autoroute::smoother::smooth_routes_within_edge;
use cypcb_autoroute::via_optimizer::{optimize_vias, BoardObstacles};
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::rules::clearance::segment_distance;
use cypcb_router::types::{RouteSegment, ViaPlacement};
use cypcb_router::RoutingStatus;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_rules::RoutingRuleSet;
use cypcb_world::components::{
    BoardCutouts, BoardEdge, BoardOutline, FootprintRef, Layer, NetConnections, PadShape,
    PinConnection,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, NetId, Position, RefDes, Rotation, Value};

const CELL: i64 = 100_000;

fn mm(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn rules() -> PresetRuleSet {
    PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"))
}

/// A 40mm by 30mm board with a pad at each of the given places, on one net.
fn board(pads: &[(f64, f64)]) -> (BoardWorld, FootprintLibrary) {
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
    for (i, (x, y)) in pads.iter().enumerate() {
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), sig));
        world.spawn_component(
            RefDes::new(format!("J{}", i + 1)),
            Value::new(""),
            Position(mm(*x, *y)),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }
    (world, library)
}

fn outline(world: &mut BoardWorld, ring: Vec<Point>) {
    let entity = world.board_entity().unwrap();
    world
        .ecs_mut()
        .entity_mut(entity)
        .insert(BoardOutline::new(ring).unwrap());
}

fn cutout(world: &mut BoardWorld, ring: Vec<Point>) {
    let entity = world.board_entity().unwrap();
    world
        .ecs_mut()
        .entity_mut(entity)
        .insert(BoardCutouts::new(vec![ring]).unwrap());
}

/// The 40 by 30 board with the top-right quarter beyond x = 20, y = 15 gone.
fn l_shape() -> Vec<Point> {
    vec![
        mm(0.0, 0.0),
        mm(40.0, 0.0),
        mm(40.0, 15.0),
        mm(20.0, 15.0),
        mm(20.0, 30.0),
        mm(0.0, 30.0),
    ]
}

fn grid(world: &mut BoardWorld, library: &FootprintLibrary) -> RoutingGrid {
    world.rebuild_spatial_index_from_library(library);
    RoutingGrid::from_board(world, library, &rules(), CELL).expect("a grid")
}

/// The cell of a point of the board, in this 0.1mm grid.
fn cell_of(x_mm: f64, y_mm: f64) -> (u32, u32) {
    ((x_mm * 10.0).floor() as u32, (y_mm * 10.0).floor() as u32)
}

fn shut_to_tracks(grid: &RoutingGrid, x_mm: f64, y_mm: f64) -> bool {
    let (x, y) = cell_of(x_mm, y_mm);
    (0..2).all(|layer| grid.cell(x, y, layer) & CELL_ZONE != 0)
}

fn shut_to_vias(grid: &RoutingGrid, x_mm: f64, y_mm: f64) -> bool {
    let (x, y) = cell_of(x_mm, y_mm);
    grid.via_forbidden(x, y, grid.via_layers(0, 1))
}

#[test]
fn the_numbers_in_this_file_are_the_tables() {
    let rules = rules();
    let constraints = rules.constraints_for_net(0);
    assert_eq!(constraints.min_edge_clearance, Nm::from_mm(0.3));
    assert_eq!(constraints.min_trace_width, Nm::from_mm(0.127));
    assert_eq!(constraints.min_via_drill, Nm::from_mm(0.3));
}

#[test]
fn a_board_with_no_outline_keeps_a_track_off_its_rectangle() {
    let (mut world, library) = board(&[(10.0, 15.0), (30.0, 15.0)]);
    assert_eq!(BoardEdge::of(&world), None);
    let grid = grid(&mut world, &library);

    // Along the left edge, half way up: cells centred 0.05 ... 0.35 from it
    // are shut and the one centred 0.45 from it is open.
    for x in [0.05, 0.15, 0.25, 0.35] {
        assert!(shut_to_tracks(&grid, x, 15.05), "x = {x}");
    }
    assert!(!shut_to_tracks(&grid, 0.45, 15.05));
    // The same from the top edge, and from the far corner.
    assert!(shut_to_tracks(&grid, 20.05, 0.35));
    assert!(!shut_to_tracks(&grid, 20.05, 0.45));
    assert!(shut_to_tracks(&grid, 39.95, 29.95));
    assert!(!shut_to_tracks(&grid, 39.55, 29.55));
}

#[test]
fn a_via_keeps_a_wider_gap_than_a_track() {
    let (mut world, library) = board(&[(10.0, 15.0), (30.0, 15.0)]);
    let grid = grid(&mut world, &library);

    // The cell centred 0.45 from the edge is open to a track and shut to a
    // via, whose ring is wider than half a track.
    assert!(!shut_to_tracks(&grid, 0.45, 15.05));
    for x in [0.05, 0.15, 0.25, 0.35, 0.45, 0.55] {
        assert!(shut_to_vias(&grid, x, 15.05), "x = {x}");
    }
    assert!(!shut_to_vias(&grid, 0.65, 15.05));
    // The middle of the board is shut to neither.
    assert!(!shut_to_tracks(&grid, 20.05, 15.05));
    assert!(!shut_to_vias(&grid, 20.05, 15.05));
}

#[test]
fn the_notch_of_an_outline_is_off_the_board() {
    let (mut world, library) = board(&[(10.0, 25.0), (35.0, 5.0)]);
    outline(&mut world, l_shape());
    let grid = grid(&mut world, &library);

    // Inside the notch, far from its edges: off the board.
    assert!(shut_to_tracks(&grid, 30.05, 22.05));
    assert!(shut_to_vias(&grid, 30.05, 22.05));
    // On the board in the arm below it, and 4.95mm from the notch's edge.
    assert!(!shut_to_tracks(&grid, 30.05, 10.05));
    // Beside the notch's lower edge (y = 15): the cells centred 0.35 and
    // 0.55 below it.
    assert!(shut_to_tracks(&grid, 30.05, 14.65));
    assert!(!shut_to_tracks(&grid, 30.05, 14.55));
    assert!(shut_to_vias(&grid, 30.05, 14.45));
    assert!(!shut_to_vias(&grid, 30.05, 14.35));
}

#[test]
fn a_cutout_is_off_the_board_and_its_rim_is_an_edge() {
    let (mut world, library) = board(&[(5.0, 15.0), (35.0, 15.0)]);
    cutout(
        &mut world,
        vec![
            mm(15.0, 10.0),
            mm(25.0, 10.0),
            mm(25.0, 20.0),
            mm(15.0, 20.0),
        ],
    );
    let grid = grid(&mut world, &library);

    assert!(
        shut_to_tracks(&grid, 20.05, 15.05),
        "the middle of the window"
    );
    assert!(
        shut_to_tracks(&grid, 14.65, 15.05),
        "0.35 outside its left rim"
    );
    assert!(!shut_to_tracks(&grid, 14.55, 15.05), "0.45 outside it");
    assert!(shut_to_vias(&grid, 14.45, 15.05));
    assert!(!shut_to_vias(&grid, 14.35, 15.05));
}

/// Every segment and via of a result keeps to the board and its gap.
fn assert_off_the_edge(world: &BoardWorld, result: &cypcb_router::RoutingResult) {
    let edge = BoardEdge::of_or_size(world).expect("an edge");
    let gap = rules().constraints_for_net(0).min_edge_clearance.0;
    assert!(!result.routes.is_empty());
    for s in &result.routes {
        let (a, b) = ([s.start.x.0, s.start.y.0], [s.end.x.0, s.end.y.0]);
        assert!(
            edge.contains(s.start) && edge.contains(s.end),
            "{s:?} is off the board"
        );
        for (c, d) in edge.edges() {
            let apart = segment_distance(a, b, [c.x.0, c.y.0], [d.x.0, d.y.0]);
            assert!(
                apart >= gap + s.width.0 / 2 - 1,
                "{s:?} is {apart} nm from an edge"
            );
        }
    }
    for v in &result.vias {
        let at = [v.position.x.0, v.position.y.0];
        assert!(edge.contains(v.position), "{v:?} is off the board");
        for (c, d) in edge.edges() {
            let apart = segment_distance(at, at, [c.x.0, c.y.0], [d.x.0, d.y.0]);
            assert!(
                apart >= gap + v.outer_diameter.0 / 2 - 1,
                "{v:?} is {apart} nm from an edge"
            );
        }
    }
}

#[test]
fn a_route_between_the_arms_of_an_l_goes_round_the_notch() {
    // The straight line from one pad to the other crosses the notch.
    let (mut world, library) = board(&[(10.0, 25.0), (35.0, 5.0)]);
    outline(&mut world, l_shape());
    world.rebuild_spatial_index_from_library(&library);
    let result = route_board(&mut world, &library, &rules(), &AutorouteConfig::default());
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "{:?}",
        result.status
    );
    assert_off_the_edge(&world, &result);
}

#[test]
fn a_route_across_a_window_goes_round_the_window() {
    let (mut world, library) = board(&[(5.0, 15.0), (35.0, 15.0)]);
    cutout(
        &mut world,
        vec![
            mm(15.0, 10.0),
            mm(25.0, 10.0),
            mm(25.0, 20.0),
            mm(15.0, 20.0),
        ],
    );
    world.rebuild_spatial_index_from_library(&library);
    let result = route_board(&mut world, &library, &rules(), &AutorouteConfig::default());
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "{:?}",
        result.status
    );
    assert_off_the_edge(&world, &result);
}

#[test]
fn a_route_along_a_plain_rectangle_keeps_off_its_rim_too() {
    // Both pads 0.5mm from the bottom edge: the straight way between them runs
    // closer to the rim than 0.3635mm would allow only if the track sits on the
    // pads' own line, so the check is on every segment.
    let (mut world, library) = board(&[(5.0, 0.8), (35.0, 0.8)]);
    world.rebuild_spatial_index_from_library(&library);
    let result = route_board(&mut world, &library, &rules(), &AutorouteConfig::default());
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "{:?}",
        result.status
    );
    assert_off_the_edge(&world, &result);
}

#[test]
fn a_row_of_cells_is_shut_exactly_as_far_as_the_rule_reaches() {
    // Along one row of a plain 40mm board, from the left edge in: the cells
    // whose centre is nearer than 0.3635mm are shut to a track and no other,
    // and those nearer than 0.6mm are shut to a via and no other. Nothing is
    // rounded out to a whole cell: 0.05 ... 0.35 are four cells and 0.05 ...
    // 0.55 are six.
    let (mut world, library) = board(&[(10.0, 15.0), (30.0, 15.0)]);
    let grid = grid(&mut world, &library);
    let row = |shut: fn(&RoutingGrid, f64, f64) -> bool| {
        (0..400)
            .filter(|n| shut(&grid, f64::from(*n) / 10.0 + 0.05, 15.05))
            .count()
    };
    // Both ends of the row are edges, so each end counts.
    assert_eq!(row(shut_to_tracks), 2 * 4);
    assert_eq!(row(shut_to_vias), 2 * 6);
}

#[test]
fn a_chamfer_that_would_cut_across_the_corner_of_a_notch_is_refused() {
    // The path comes down the arm 0.5mm left of the notch and turns right 0.5mm
    // above its floor; the corner of the notch is at (20, 15). Cutting the
    // bend with a 0.5mm chamfer puts the middle of the cut 0.354mm from that
    // corner, which is nearer than a 0.127mm track may come to an edge.
    let (mut world, _) = board(&[(10.0, 25.0), (35.0, 5.0)]);
    outline(&mut world, l_shape());
    let guard = EdgeGuard::of_board(&world, Nm::from_mm(0.3)).expect("an edge");
    let path = vec![
        RouteSegment::new(
            NetId::new(1),
            Layer::TopCopper,
            Nm::from_mm(0.127),
            mm(19.5, 25.0),
            mm(19.5, 14.5),
        ),
        RouteSegment::new(
            NetId::new(1),
            Layer::TopCopper,
            Nm::from_mm(0.127),
            mm(19.5, 14.5),
            mm(30.0, 14.5),
        ),
    ];
    let slanted = |segments: &[RouteSegment]| {
        segments
            .iter()
            .any(|s| s.start.x != s.end.x && s.start.y != s.end.y)
    };

    let free = smooth_routes_within_edge(&path, &[], &[], &[], Nm(0), 0.5, None);
    assert!(
        slanted(&free),
        "the chamfer is cut when nothing says otherwise"
    );

    let kept = smooth_routes_within_edge(&path, &[], &[], &[], Nm(0), 0.5, Some(&guard));
    assert!(!slanted(&kept), "{kept:?}");
    assert_eq!(kept.len(), 2);
}

#[test]
fn a_pair_of_vias_is_not_joined_by_a_segment_across_a_notch() {
    // The track goes down at (15, 22), runs under the notch's corner on the
    // bottom layer and comes up at (26, 10). The direct way between the two on
    // the top layer crosses the notch, so it is not a way.
    let (mut world, library) = board(&[(10.0, 25.0), (35.0, 5.0)]);
    outline(&mut world, l_shape());
    let net = NetId::new(1);
    let via = |x: f64, y: f64, from: Layer, to: Layer| {
        ViaPlacement::new(net, mm(x, y), Nm::from_mm(0.3), from, to)
    };
    let segments = || {
        let wide = Nm::from_mm(0.127);
        vec![
            RouteSegment::new(net, Layer::TopCopper, wide, mm(10.0, 22.0), mm(15.0, 22.0)),
            RouteSegment::new(
                net,
                Layer::BottomCopper,
                wide,
                mm(15.0, 22.0),
                mm(26.0, 10.0),
            ),
            RouteSegment::new(net, Layer::TopCopper, wide, mm(26.0, 10.0), mm(30.0, 10.0)),
        ]
    };
    let vias = || {
        vec![
            via(15.0, 22.0, Layer::TopCopper, Layer::BottomCopper),
            via(26.0, 10.0, Layer::BottomCopper, Layer::TopCopper),
        ]
    };
    let clearance = Nm::from_mm(0.127);

    let blind = BoardObstacles::from_board(&mut world, &library);
    let (_, joined) = optimize_vias(segments(), vias(), &blind, clearance, Nm(0));
    assert_eq!(joined.len(), 0, "with no edge the pair is joined");

    let aware = BoardObstacles::from_board_with_edge(&mut world, &library, Nm::from_mm(0.3));
    let (kept_segments, kept) = optimize_vias(segments(), vias(), &aware, clearance, Nm(0));
    assert_eq!(kept.len(), 2, "the notch keeps the pair");
    assert_eq!(kept_segments.len(), 3);
}

#[test]
fn a_via_set_down_nearer_than_its_ring_and_the_gap_is_refused() {
    let (world, _) = board(&[(10.0, 15.0), (30.0, 15.0)]);
    let guard = EdgeGuard::of_board(&world, Nm::from_mm(0.3)).expect("an edge");
    let ring = Nm::from_mm(0.6);
    let at = |x: f64| {
        let p = mm(x, 15.0);
        [p.x.0, p.y.0]
    };
    // A 0.6mm ring and the 0.3mm gap: the centre needs 0.6mm.
    assert!(!guard.allows(at(0.59), at(0.59), ring));
    assert!(guard.allows(at(0.61), at(0.61), ring));
    // Off the board is no better.
    assert!(!guard.allows(at(-1.0), at(-1.0), ring));
}
