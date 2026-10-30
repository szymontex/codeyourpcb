//! A pour held to the board's edge is still one sheet where it is one sheet.
//!
//! `cargo test -p cypcb-drc --test a_pour_cut_by_a_cutout_is_one_sheet`
//!
//! Holding a pour to a board edge leaves the rectangles it spared as
//! rectangles and makes convex pieces of the ones it cut. `PourIslandRule`
//! measured rectangles and convex pieces apart, so a piece cut by a window and
//! its neighbour the cut spared were two sheets, and the one with no spoke on
//! it was reported as copper that reaches no pad - on 15 boards of the 132 the
//! repository holds (252 rows), before this was measured as one.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{DrcRule, PourIslandRule};
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::components::{
    BoardCutouts, FootprintRef, Layer, NetConnections, PadShape, PinConnection,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, Position, RefDes, Rotation, Value};

fn at(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

/// A 40 x 30mm board with a 10 x 10mm window in it, a ground pad well to the
/// left of the window, and a ground pour over all of it.
fn board(window: bool) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("w".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    if window {
        let entity = world.board_entity().unwrap();
        let cutouts = BoardCutouts::new(vec![vec![
            at(15.0, 10.0),
            at(25.0, 10.0),
            at(25.0, 20.0),
            at(15.0, 20.0),
        ]])
        .unwrap();
        world.ecs_mut().entity_mut(entity).insert(cutouts);
    }
    let gnd = world.intern_net("GND");

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
    world.set_footprints(library);

    let mut connections = NetConnections::new();
    connections.add(PinConnection::new("1".to_string(), gnd));
    world.spawn_component(
        RefDes::new("J1"),
        Value::new(""),
        Position(at(5.0, 15.0)),
        Rotation::ZERO,
        FootprintRef::new("PAD1"),
        connections,
    );
    world.spawn_entity(Zone {
        bounds: Rect::new(at(1.0, 1.0), at(39.0, 29.0)),
        kind: ZoneKind::CopperPour,
        layer_mask: Layer::TopCopper.to_copper_mask(),
        name: Some("gnd".to_string()),
        net: Some(gnd),
    });
    world.rebuild_spatial_index_from_library(&world.footprints().clone());
    world
}

#[test]
fn a_pour_round_a_window_is_one_sheet_that_reaches_its_pad() {
    let mut world = board(true);
    let rows = PourIslandRule.check(&mut world, &DesignRules::jlcpcb_2layer());
    assert!(rows.is_empty(), "{rows:?}");
}

#[test]
fn control_the_window_is_what_cuts_the_pour() {
    // The same pour on the board without the window is whole rectangles, and
    // the window makes convex pieces of the rectangles it cuts: the fill has
    // both kinds, which is what the rule has to read together.
    let rules = DesignRules::jlcpcb_2layer();
    let options = cypcb_core::pour::PourOptions {
        clearance: rules.min_copper_pour_clearance,
        edge_clearance: rules.min_edge_clearance,
        thermal_gap: rules.thermal_relief_gap,
        spoke_width: rules.thermal_relief_spoke_width,
    };
    let fill = |window: bool| {
        let mut world = board(window);
        let library = world.footprints().clone();
        let (_, zone) = world
            .zones()
            .into_iter()
            .find(|(_, z)| z.kind == ZoneKind::CopperPour)
            .unwrap();
        cypcb_world::copper::fill_zone(
            &mut world,
            &library,
            Layer::TopCopper,
            &zone,
            None,
            &options,
        )
    };
    let (plain, windowed) = (fill(false), fill(true));
    assert!(plain.shapes.is_empty());
    assert!(
        !windowed.shapes.is_empty(),
        "the window cut no convex piece"
    );
    assert!(
        !windowed.pieces.is_empty(),
        "the window cut every rectangle"
    );
}
