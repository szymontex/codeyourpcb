//! The router keeps out of the cells a polygon keepout covers, and of no others.
//!
//! `cargo test -p cypcb-autoroute --test a_keepout_that_is_a_polygon_blocks_its_cells`
//!
//! A keepout is marked on the routing grid by the cells it covers. For a
//! rectangle that is the cells of its box. A keepout drawn as an L read from
//! KiCad was not in the board, so the router ran straight through it; marked
//! by its box it would have shut the notch, which is the part of the L the
//! designer left open. What is held: the cells of the ring and the clearance
//! round it are blocked, the cells of the notch beyond that are free, and the
//! clearance field the cost model reads agrees.

use cypcb_autoroute::clearance_field::ClearanceField;
use cypcb_autoroute::grid::{RoutingGrid, CELL_ZONE};
use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::{preset_for_world, ruleset_for_world};
use cypcb_world::components::zone::{Zone, ZoneOutline};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

const RESOLUTION: i64 = 100_000;

fn p(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

/// A 40 x 30 board with an L-shaped keepout on the top layer, 10 x 10 with the
/// upper right quarter cut away - or the 10 x 10 square.
fn board(l: bool) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    if l {
        let ring = ZoneOutline::new(vec![
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 5.0),
            p(5.0, 5.0),
            p(5.0, 10.0),
            p(0.0, 10.0),
        ])
        .expect("an L");
        world.spawn_outlined_zone(Zone::keepout(Rect::default(), 0b01), ring);
    } else {
        world.spawn_entity((Zone::keepout(Rect::new(p(0.0, 0.0), p(10.0, 10.0)), 0b01),));
    }
    world
}

fn grid_of(world: &mut BoardWorld, library: &FootprintLibrary) -> RoutingGrid {
    let preset = preset_for_world(
        cypcb_rules::presets::RulesPreset::JlcpcbStandard2Layer,
        world,
    );
    let rules = ruleset_for_world(preset, world);
    RoutingGrid::from_board(world, library, &rules, RESOLUTION).expect("the board has a grid")
}

fn blocked(grid: &RoutingGrid, x_mm: f64, y_mm: f64, layer: usize) -> bool {
    let (gx, gy) = grid.nm_to_grid(p(x_mm, y_mm));
    grid.cell(gx, gy, layer) & CELL_ZONE != 0
}

#[test]
fn the_cells_inside_the_ring_are_blocked() {
    let mut world = board(true);
    let grid = grid_of(&mut world, &FootprintLibrary::new());
    assert!(blocked(&grid, 2.0, 2.0, 0), "the corner of the L");
    assert!(blocked(&grid, 8.0, 2.0, 0), "the foot");
    assert!(blocked(&grid, 2.0, 8.0, 0), "the stem");
}

#[test]
fn the_cells_in_the_notch_are_free() {
    let mut world = board(true);
    let grid = grid_of(&mut world, &FootprintLibrary::new());
    assert!(
        !blocked(&grid, 8.0, 8.0, 0),
        "the middle of the notch is inside the box and outside the L"
    );
    assert!(!blocked(&grid, 9.0, 9.0, 0));
}

#[test]
fn the_cells_beside_the_ring_are_blocked_by_the_clearance_and_no_further() {
    let mut world = board(true);
    let grid = grid_of(&mut world, &FootprintLibrary::new());
    assert!(
        blocked(&grid, 5.1, 6.0, 0),
        "the notch's wall, a cell from the edge"
    );
    assert!(
        blocked(&grid, 10.1, 2.0, 0),
        "outside the foot, a cell from the edge"
    );
    assert!(!blocked(&grid, 12.0, 2.0, 0), "two millimetres off");
    assert!(!blocked(&grid, 20.0, 20.0, 0));
}

#[test]
fn only_the_layers_the_zone_is_on_are_blocked() {
    let mut world = board(true);
    let grid = grid_of(&mut world, &FootprintLibrary::new());
    assert!(blocked(&grid, 2.0, 2.0, 0), "top");
    assert!(!blocked(&grid, 2.0, 2.0, 1), "not the bottom");
}

#[test]
fn control_a_rectangle_blocks_its_whole_box() {
    let mut world = board(false);
    let grid = grid_of(&mut world, &FootprintLibrary::new());
    assert!(
        blocked(&grid, 8.0, 8.0, 0),
        "what is the notch of the L is inside the square"
    );
    assert!(blocked(&grid, 2.0, 2.0, 0));
}

#[test]
fn the_clearance_field_agrees_with_the_grid() {
    let library = FootprintLibrary::new();
    let mut world = board(true);
    let grid = grid_of(&mut world, &library);
    let field = ClearanceField::from_board(&mut world, &library, &grid);
    let at = |x: f64, y: f64| {
        let (gx, gy) = grid.nm_to_grid(p(x, y));
        field.distance_nm(gx, gy, 0)
    };
    assert_eq!(
        at(2.0, 2.0),
        Some(0),
        "inside the ring is copper to the field"
    );
    assert!(at(8.0, 8.0).expect("a distance") > 0, "the notch is not");
    assert!(
        at(8.0, 8.0) < at(9.5, 9.5),
        "and it is further from the L the deeper it is in the notch"
    );
}
