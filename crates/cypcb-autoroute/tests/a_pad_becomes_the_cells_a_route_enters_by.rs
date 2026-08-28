//! A pad becomes the cells the router must leave open.
//!
//! `cargo test -p cypcb-autoroute --test a_pad_becomes_the_cells_a_route_enters_by`
//!
//! Three helpers convert between a pad and the grid a router searches, and
//! until this nothing named any of them: `pad_to_grid_node` says which cell a
//! route aims at, `pad_to_zone` says how much of the obstacle around it a
//! route may still enter, and `pad_to_zone_with_margin` is the same with the
//! margin stated. They are the tail of V1's census - no number they produce
//! reaches a person - but every route in this project starts and ends at one
//! of their answers, and a board with a pad the router cannot enter is a board
//! with a net it cannot finish.

use cypcb_autoroute::grid::RoutingGrid;
use cypcb_autoroute::orchestrator::{
    pad_to_grid_node, pad_to_zone, pad_to_zone_with_margin, PadTarget,
    DEFAULT_PAD_ZONE_MARGIN_CELLS,
};
use cypcb_core::{Nm, Point};
use cypcb_parser::parse;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// A plain 40mm by 30mm two-layer board, so the grid is the board's own.
const BOARD: &str = "version 1\n\nboard b {\n    size 40mm x 30mm\n    layers 2\n}\n";

fn grid() -> RoutingGrid {
    let parsed = parse(BOARD);
    assert!(parsed.is_ok(), "the board parses: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let synced = sync_ast_to_world(&parsed.value, BOARD, &mut world, &mut library);
    assert!(!synced.has_errors(), "the board syncs: {:?}", synced.errors);

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("a preset"));
    // A quarter-millimetre cell, which is what the dense fixtures route on and
    // what makes the arithmetic below readable.
    RoutingGrid::from_board(&mut world, &library, &rules, 250_000).expect("a grid")
}

fn pad_at(x: f64, y: f64, layer_mask: u32, size_mm: (f64, f64)) -> PadTarget {
    PadTarget {
        position: Point::from_mm(x, y),
        layer_mask,
        pad_size: (Nm::from_mm(size_mm.0), Nm::from_mm(size_mm.1)),
        pin: "1".to_string(),
    }
}

#[test]
fn the_cell_a_route_aims_at_is_the_one_the_pad_sits_in() {
    let grid = grid();
    let node = pad_to_grid_node(&grid, &pad_at(10.0, 5.0, 0b01, (1.0, 1.0)));

    // 10mm and 5mm on a 0.25mm grid, and the top layer because that is the
    // only one this pad is on.
    assert_eq!(node, (40, 20, 0), "the pad's own cell, on its own layer");
}

#[test]
fn a_pad_is_entered_from_the_lowest_layer_it_reaches() {
    // Bit 0 is the top face, so a through-hole pad is approached from the top
    // rather than from the middle of the board. A router that picked another
    // layer would drill a via to reach copper it was already standing on.
    let grid = grid();
    let everywhere = pad_to_grid_node(&grid, &pad_at(10.0, 5.0, 0b1111, (1.0, 1.0)));
    assert_eq!(
        everywhere.2, 0,
        "a pad on every layer is entered at the top"
    );

    let lower = pad_to_grid_node(&grid, &pad_at(10.0, 5.0, 0b0110, (1.0, 1.0)));
    assert_eq!(
        lower.2, 1,
        "and one on the second and third layers at the second, not the third"
    );
}

#[test]
fn the_zone_covers_the_pads_own_copper_and_a_margin_beyond_it() {
    // The pad is an obstacle to everything except the route that lands on it,
    // and the zone is the hole in that obstacle. Too small and no route can
    // reach the pad; too large and a route can cut the corner through copper
    // it should have gone around.
    let grid = grid();
    let zone = pad_to_zone(&grid, &pad_at(10.0, 5.0, 0b01, (2.0, 1.0)));

    assert_eq!((zone.cx, zone.cy), (40, 20), "centred on the pad");
    // The longer side is 2mm, so the pad reaches 1mm - four cells - from its
    // centre, and the default margin opens three more.
    assert_eq!(
        zone.radius,
        4 + DEFAULT_PAD_ZONE_MARGIN_CELLS,
        "the pad's own reach plus the margin"
    );
}

#[test]
fn a_pad_that_does_not_fill_a_cell_still_gets_one() {
    // The reach is rounded up, not down: a 0.1mm pad on a 0.25mm grid covers
    // less than half a cell, and a zone of radius zero plus the margin would
    // be a pad the router enters by luck.
    let grid = grid();
    let tiny = pad_to_zone(&grid, &pad_at(10.0, 5.0, 0b01, (0.1, 0.1)));
    assert_eq!(tiny.radius, 1 + DEFAULT_PAD_ZONE_MARGIN_CELLS);
}

#[test]
fn the_margin_is_the_one_that_was_asked_for() {
    // `pad_to_zone` is `pad_to_zone_with_margin` at the default, and the sweep
    // that chose that default lives on the constant. A caller that states a
    // margin gets it rather than the default.
    let grid = grid();
    let pad = pad_at(10.0, 5.0, 0b01, (2.0, 1.0));

    let default = pad_to_zone(&grid, &pad);
    let stated = pad_to_zone_with_margin(&grid, &pad, DEFAULT_PAD_ZONE_MARGIN_CELLS);
    assert_eq!(
        (default.cx, default.cy, default.radius),
        (stated.cx, stated.cy, stated.radius),
        "the default is the one the constant states"
    );
    assert_eq!(
        pad_to_zone_with_margin(&grid, &pad, 0).radius,
        4,
        "and no margin is the pad's own reach and nothing else"
    );
    assert_eq!(pad_to_zone_with_margin(&grid, &pad, 7).radius, 11);
}
