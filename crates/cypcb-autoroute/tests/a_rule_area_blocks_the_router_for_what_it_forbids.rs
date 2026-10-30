//! A rule area blocks the router for the kinds it forbids.
//!
//! `cargo test -p cypcb-autoroute --test a_rule_area_blocks_the_router_for_what_it_forbids`
//!
//! The router read every keepout as a place nothing may go. A rule area that
//! forbids only vias stops a via being placed and leaves a track free to run
//! through; one that forbids tracks shuts the cells; one that forbids only pads,
//! footprints or copper pour is not something the router lays down and changes
//! nothing it does.
//!
//! J1 and J2 are on one net, 20mm apart on the top layer. The "wall" is a trace
//! on no net across the whole height of the board between them on the top layer,
//! which leaves the only way across under it, through two vias.

use cypcb_autoroute::grid::{RoutingGrid, CELL_ZONE};
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::{Nm, Point, Rect};
use cypcb_router::RoutingStatus;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::zone::{KeepoutKind, KeepoutKinds, Zone};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, Position, RefDes, Rotation, Value};

fn mm(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn board(wall: bool) -> (BoardWorld, FootprintLibrary) {
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
            Position(mm(x, 15.0)),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }
    if wall {
        let mut wall = Trace::new(sig);
        wall.net_id = None;
        wall.layer = Layer::TopCopper;
        wall.width = Nm::from_mm(0.25);
        wall.source = TraceSource::Manual;
        wall.add_segment(TraceSegment::new(mm(20.0, 0.0), mm(20.0, 30.0)));
        world.ecs_mut().spawn((wall,));
    }
    (world, library)
}

/// A rule area across the whole height of the board, `x0` to `x1`, on both
/// layers. `None` is a keepout with no component: all five kinds.
fn area(world: &mut BoardWorld, x0: f64, x1: f64, kinds: Option<&[KeepoutKind]>) {
    let zone = Zone::keepout(Rect::new(mm(x0, 0.0), mm(x1, 30.0)), 0b11);
    match kinds {
        Some(kinds) => world.spawn_entity((zone, KeepoutKinds::of_kinds(kinds))),
        None => world.spawn_entity((zone,)),
    };
}

/// The status and the number of vias the router chose.
fn routed(mut world: BoardWorld, library: FootprintLibrary) -> (bool, usize) {
    world.rebuild_spatial_index_from_library(&library);
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    (
        matches!(result.status, RoutingStatus::Complete),
        result.vias.len(),
    )
}

#[test]
fn the_wall_alone_is_crossed_through_two_vias() {
    let (world, library) = board(true);
    let (complete, vias) = routed(world, library);
    assert!(complete);
    assert!(vias >= 2, "{vias}");
}

#[test]
fn an_area_that_forbids_vias_leaves_no_way_under_the_wall() {
    // The area covers the whole board, so no via fits anywhere: a way round
    // the wall by a via beside either pad would be a way across.
    let (mut world, library) = board(true);
    area(&mut world, 0.0, 40.0, Some(&[KeepoutKind::Vias]));
    let (complete, _) = routed(world, library);
    assert!(!complete, "a via was placed in an area that forbids vias");
}

#[test]
fn an_area_that_forbids_vias_leaves_a_track_free_to_cross_it() {
    // No wall: the straight track between the pads runs through the area and
    // needs no via.
    let (mut world, library) = board(false);
    area(&mut world, 15.0, 25.0, Some(&[KeepoutKind::Vias]));
    let (complete, vias) = routed(world, library);
    assert!(complete);
    assert_eq!(vias, 0);
}

#[test]
fn an_area_that_forbids_tracks_shuts_the_way() {
    let (mut world, library) = board(false);
    area(&mut world, 15.0, 25.0, Some(&[KeepoutKind::Tracks]));
    let (complete, _) = routed(world, library);
    assert!(!complete, "a track crossed an area that forbids tracks");
}

#[test]
fn a_keepout_with_no_component_shuts_the_way_as_it_always_did() {
    let (mut world, library) = board(false);
    area(&mut world, 15.0, 25.0, None);
    let (complete, _) = routed(world, library);
    assert!(!complete);
}

#[test]
fn an_area_that_forbids_what_the_router_does_not_lay_down_changes_nothing() {
    for kind in [
        KeepoutKind::Pads,
        KeepoutKind::Footprints,
        KeepoutKind::Pour,
    ] {
        let (mut world, library) = board(false);
        area(&mut world, 15.0, 25.0, Some(&[kind]));
        let (complete, vias) = routed(world, library);
        assert!(complete, "{kind:?}");
        assert_eq!(vias, 0, "{kind:?}");
    }
}

fn grid_of(world: &mut BoardWorld, library: &FootprintLibrary) -> RoutingGrid {
    let preset = cypcb_drc::preset_for_world(RulesPreset::JlcpcbStandard2Layer, world);
    let rules = cypcb_drc::ruleset_for_world(preset, world);
    RoutingGrid::from_board(world, library, &rules, 100_000).expect("the board has a grid")
}

#[test]
fn the_grid_marks_cells_for_tracks_and_keeps_vias_apart() {
    let at = |grid: &RoutingGrid| grid.nm_to_grid(mm(20.0, 5.0));
    let marked = |kinds: Option<&[KeepoutKind]>| {
        let (mut world, library) = board(false);
        area(&mut world, 15.0, 25.0, kinds);
        let grid = grid_of(&mut world, &library);
        let (gx, gy) = at(&grid);
        let both = grid.via_layers(0, 1);
        (
            grid.cell(gx, gy, 0) & CELL_ZONE != 0,
            grid.via_forbidden(gx, gy, both),
        )
    };
    assert_eq!(marked(Some(&[KeepoutKind::Tracks])), (true, false));
    assert_eq!(marked(Some(&[KeepoutKind::Vias])), (false, true));
    assert_eq!(marked(Some(&[KeepoutKind::Pour])), (false, false));
    assert_eq!(marked(None), (true, false));
}

/// The same area drawn as a ring of corners, which is how every area read from
/// KiCad arrives.
fn outlined_area(world: &mut BoardWorld, x0: f64, x1: f64, kinds: Option<&[KeepoutKind]>) {
    let ring = cypcb_world::components::zone::ZoneOutline::new(vec![
        mm(x0, 0.0),
        mm(x1, 0.0),
        mm(x1, 30.0),
        mm(x0, 30.0),
    ])
    .expect("a rectangle");
    let entity = world.spawn_outlined_zone(Zone::keepout(Rect::default(), 0b11), ring);
    if let Some(kinds) = kinds {
        world
            .ecs_mut()
            .entity_mut(entity)
            .insert(KeepoutKinds::of_kinds(kinds));
    }
}

#[test]
fn a_ring_that_forbids_tracks_shuts_the_way() {
    let (mut world, library) = board(false);
    outlined_area(&mut world, 15.0, 25.0, Some(&[KeepoutKind::Tracks]));
    let (complete, _) = routed(world, library);
    assert!(!complete, "a track crossed an area that forbids tracks");
}

#[test]
fn a_ring_that_forbids_vias_leaves_a_track_free_and_no_way_under_a_wall() {
    let (mut world, library) = board(false);
    outlined_area(&mut world, 15.0, 25.0, Some(&[KeepoutKind::Vias]));
    let (complete, vias) = routed(world, library);
    assert!(complete);
    assert_eq!(vias, 0);

    let (mut world, library) = board(true);
    outlined_area(&mut world, 0.0, 40.0, Some(&[KeepoutKind::Vias]));
    let (complete, _) = routed(world, library);
    assert!(!complete, "a via was placed in an area that forbids vias");
}

#[test]
fn a_ring_that_forbids_pour_changes_nothing() {
    let (mut world, library) = board(false);
    outlined_area(&mut world, 15.0, 25.0, Some(&[KeepoutKind::Pour]));
    let (complete, vias) = routed(world, library);
    assert!(complete);
    assert_eq!(vias, 0);
}
