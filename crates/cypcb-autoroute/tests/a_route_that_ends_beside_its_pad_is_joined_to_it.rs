//! A route that ends beside its pad is joined to the pad.
//!
//! `cargo test -p cypcb-autoroute --test a_route_that_ends_beside_its_pad_is_joined_to_it`
//!
//! A route ends at the centre of the cell that holds its pin. On a grid whose
//! cells are wider than the pad - 0.6mm here, 0.627mm on `esp32_starter` - that
//! centre can be beside the pad and not on it, and the connection counted as
//! routed while the pin had no copper of its net on it: `J1.A5` of the USB-C
//! receptacle, left open by a change that moved no copper near it.
//!
//! Two 0.2mm pads, each 0.25mm along both axes from the centre of its cell.

use cypcb_autoroute::edge_guard::EdgeGuard;
use cypcb_autoroute::postprocess::join_pins;
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{DrcRule, UnroutedPinRule};
use cypcb_drc::{run_drc, ViolationKind};
use cypcb_router::apply_routes;
use cypcb_router::types::{RouteSegment, RoutingResult};
use cypcb_router::RoutingStatus;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, NetId, Position, RefDes, Rotation, Value};

const CELL: i64 = 600_000;

fn board(pads: &[(f64, f64)]) -> (BoardWorld, FootprintLibrary) {
    let named: Vec<(f64, f64, &str)> = pads.iter().map(|(x, y)| (*x, *y, "SIG")).collect();
    board_of(&named)
}

/// The same board with each pad on a net of its own naming.
fn board_of(pads: &[(f64, f64, &str)]) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
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
                size: (Nm::from_mm(0.2), Nm::from_mm(0.2)),
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
    for (i, (x, y, net)) in pads.iter().enumerate() {
        let net = world.intern_net(net);
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), net));
        world.spawn_component(
            RefDes::new(format!("J{}", i + 1)),
            Value::new(""),
            Position(Point::from_mm(*x, *y)),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );
    }
    (world, library)
}

fn open_pins(pads: &[(f64, f64)]) -> usize {
    let (mut world, library) = board(pads);
    world.rebuild_spatial_index_from_library(&library);
    let preset = RulesPreset::from_name("jlcpcb").expect("the preset");
    let rules = PresetRuleSet::new(preset);
    let config = AutorouteConfig {
        grid_resolution_nm: Some(CELL),
        ..AutorouteConfig::default()
    };
    let result = route_board(&mut world, &library, &rules, &config);
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "{:?}",
        result.status
    );
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);
    let design_rules = DesignRules::from_constraints(&preset.constraints());
    UnroutedPinRule.check(&mut world, &design_rules).len()
}

#[test]
fn a_pad_the_cell_centre_misses_is_reached_all_the_same() {
    // The cells are 0.6mm, so a pad at 4.85 is in the cell centred 5.1: 0.25mm
    // from its centre, with 0.1mm of pad and 0.0635mm of track between.
    assert_eq!(open_pins(&[(4.85, 4.85), (24.85, 4.85)]), 0);
}

/// A route of net `SIG` that ends at the centre of the 0.6mm cell holding the
/// pad at (4.85, 4.85): (5.1, 5.1), 0.25mm from the pad's centre along both axes.
fn route_ending_beside_the_pad() -> Vec<RouteSegment> {
    vec![RouteSegment::new(
        NetId::new(0),
        Layer::TopCopper,
        Nm::from_mm(0.127),
        Point::from_mm(5.1, 12.0),
        Point::from_mm(5.1, 5.1),
    )]
}

fn joined(pads: &[(f64, f64, &str)], edge: Option<f64>) -> usize {
    let (mut world, library) = board_of(pads);
    let found = cypcb_drc::rules::net_pads(&mut world, &library);
    let guard = edge.and_then(|gap| EdgeGuard::of_board(&world, Nm::from_mm(gap)));
    let mut segments = route_ending_beside_the_pad();
    join_pins(
        &mut segments,
        &[],
        &found,
        Nm(CELL),
        Nm::from_mm(0.127),
        guard.as_ref(),
    )
}

#[test]
fn a_join_that_would_pass_closer_to_another_nets_pad_than_the_clearance_is_not_laid() {
    // The pad at (5.1, 4.65) is another net's. From the end (5.1, 5.1) it is
    // 0.35mm away, but the join runs on to (4.85, 4.85), 0.18mm from that pad's
    // corner at (5.0, 4.75): nearer than the end it starts from, and nearer than
    // the 0.127mm and half a track the fab asks for, which is 0.1905mm.
    let pads = [(4.85, 4.85, "SIG"), (5.1, 4.65, "OTHER")];
    assert_eq!(joined(&pads, None), 0);
    // The same join with that pad moved out of its way is laid.
    let clear = [(4.85, 4.85, "SIG"), (9.1, 4.65, "OTHER")];
    assert_eq!(joined(&clear, None), 1);
}

#[test]
fn a_join_does_not_run_to_a_pad_in_the_band_along_the_edge() {
    // The pad sits 0.1mm from the left edge, so a join to its centre would keep
    // 0.1mm from the edge where 0.3mm and half a track are asked.
    let pads = [(0.1, 5.0, "SIG")];
    let (mut world, library) = board_of(&pads);
    let found = cypcb_drc::rules::net_pads(&mut world, &library);
    let guard = EdgeGuard::of_board(&world, Nm::from_mm(0.3)).expect("an edge");
    let end = || {
        vec![RouteSegment::new(
            NetId::new(0),
            Layer::TopCopper,
            Nm::from_mm(0.127),
            Point::from_mm(0.5, 12.0),
            Point::from_mm(0.5, 5.0),
        )]
    };
    let lay = |guard: Option<&EdgeGuard>| {
        let mut segments = end();
        join_pins(
            &mut segments,
            &[],
            &found,
            Nm(CELL),
            Nm::from_mm(0.127),
            guard,
        )
    };
    assert_eq!(lay(None), 1, "nothing but the edge stops it");
    assert_eq!(lay(Some(&guard)), 0);
}

/// The clearance rows the checker reports for these segments on this board.
fn clearance_rows(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    segments: &[RouteSegment],
) -> usize {
    apply_routes(
        world,
        &RoutingResult::complete(segments.to_vec(), Vec::new()),
    );
    world.rebuild_spatial_index_from_library(library);
    run_drc(world, &DesignRules::jlcpcb_2layer())
        .violations
        .iter()
        .filter(|violation| violation.kind == ViolationKind::Clearance)
        .count()
}

#[test]
fn a_join_that_starts_beside_a_neighbour_goes_away_from_it_and_adds_no_clearance_row() {
    // The end (5.1, 5.1) is already 0.141mm from the corner (5.2, 5.2) of the
    // pad of another net at (5.3, 5.3), where 0.1905mm is asked (0.127mm and
    // half a track). The join runs on to (4.85, 4.85), away from that pad, so it
    // never comes nearer than the end it starts from. The full clearance alone
    // would refuse it, and then the pin stays open for a join that makes the
    // board no worse.
    let pads = [(4.85, 4.85, "SIG"), (5.3, 5.3, "OTHER")];
    let (mut world, library) = board_of(&pads);
    let found = cypcb_drc::rules::net_pads(&mut world, &library);
    let mut segments = route_ending_beside_the_pad();

    let before = clearance_rows(&mut world, &library, &segments);
    let laid = join_pins(
        &mut segments,
        &[],
        &found,
        Nm(CELL),
        Nm::from_mm(0.127),
        None,
    );
    let after = clearance_rows(&mut world, &library, &segments);

    assert_eq!(laid, 1, "the join is laid");
    assert!(
        before > 0,
        "the end is beside the neighbour before the join"
    );
    assert!(
        after <= before,
        "{before} clearance rows before the join, {after} after"
    );
}
