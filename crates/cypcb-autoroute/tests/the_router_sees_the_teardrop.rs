//! The router keeps its distance from a teardrop, as it does from the track.
//!
//! `cargo test -p cypcb-autoroute --test the_router_sees_the_teardrop`
//!
//! A teardrop is copper in the board, wider than its track near the land. A
//! router that read only tracks and lands would lay a new track through the
//! fillet's flank. One round land 1.6mm across at (10, 10), a track from its
//! centre to the right, and a point at (10.9, 10.4): inside the fillet, outside
//! the land (0.985mm from its centre) and outside the track (0.4mm from its
//! centreline).

use cypcb_autoroute::clearance_field::ClearanceField;
use cypcb_autoroute::grid::{RoutingGrid, CELL_TRACE};
use cypcb_autoroute::AutorouteConfig;
use cypcb_core::{Nm, Point, Rect};
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, PinConnection, Position, RefDes, Rotation,
    Teardrops, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

fn board(asked: bool) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let size = Nm::from_mm(1.6);
    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "LAND".into(),
        description: String::new(),
        bounds: Rect::from_center_size(Point::ORIGIN, (size, size)),
        courtyard: Rect::from_center_size(Point::ORIGIN, (size, size)),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".into(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (size, size),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        }],
    });
    world.set_footprints(library.clone());
    let net = world.intern_net("A");
    let mut connections = NetConnections::new();
    connections.add(PinConnection::new("1".to_string(), net));
    world.spawn_component(
        RefDes::new("R1"),
        Value::new(""),
        Position(Point::from_mm(10.0, 10.0)),
        Rotation::ZERO,
        FootprintRef::new("LAND"),
        connections,
    );
    let mut trace = Trace::new(net);
    trace.layer = Layer::TopCopper;
    trace.width = Nm::from_mm(0.2);
    trace.source = TraceSource::Manual;
    trace.add_segment(TraceSegment::new(
        Point::from_mm(10.0, 10.0),
        Point::from_mm(16.0, 10.0),
    ));
    world.ecs_mut().spawn((trace, net));
    if asked {
        assert!(world.set_teardrops(Teardrops::default()));
    }
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

fn rules() -> PresetRuleSet {
    PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"))
}

/// A fine grid, so a cell stands for a point of the board.
const RESOLUTION: i64 = 50_000;

#[test]
fn the_clearance_field_has_copper_in_the_fillet() {
    for (asked, copper) in [(false, false), (true, true)] {
        let (mut world, library) = board(asked);
        let grid =
            RoutingGrid::from_board(&mut world, &library, &rules(), RESOLUTION).expect("a grid");
        let field = ClearanceField::from_board(&mut world, &library, &grid);
        let (x, y) = (
            grid.nm_to_grid_x(Nm::from_mm(10.9).raw()),
            grid.nm_to_grid_y(Nm::from_mm(10.4).raw()),
        );
        assert_eq!(field.is_copper(x, y, 0), copper, "teardrops asked: {asked}");
    }
}

#[test]
fn the_grid_blocks_the_fillet() {
    for (asked, blocked) in [(false, false), (true, true)] {
        let (mut world, library) = board(asked);
        let grid =
            RoutingGrid::from_board(&mut world, &library, &rules(), RESOLUTION).expect("a grid");
        let (x, y) = (
            grid.nm_to_grid_x(Nm::from_mm(10.9).raw()),
            grid.nm_to_grid_y(Nm::from_mm(10.4).raw()),
        );
        // The track's own bloat is its half width plus the clearance: the point
        // is further than that from the centreline, so only the fillet can
        // mark it.
        assert_eq!(
            grid.cell(x, y, 0) & CELL_TRACE != 0,
            blocked,
            "teardrops asked: {asked}"
        );
    }
}

#[test]
fn the_config_resolves_a_grid() {
    // The default resolution is coarser than the one above; the test above
    // would hide nothing if this were not true.
    assert!(AutorouteConfig::default().resolve_grid_resolution(&rules()) > 0);
}
