//! A* pathfinder for PCB routing on a discretized grid.
//!
//! Uses the `pathfinding` crate's [`astar`] implementation with a
//! PCB-aware cost function from [`RoutingCost`]. The pathfinder operates
//! on a single net at a time, finding a path between two pad positions
//! that may cross layers via vias.
//!
//! # Grid Nodes
//!
//! A [`GridNode`] is `(grid_x, grid_y, layer_index)`. The pathfinder
//! generates 8-directional neighbors on the same layer plus vertical
//! (via) transitions to adjacent layers at the current position.

use crate::cost::RoutingCost;
use crate::grid::RoutingGrid;
use pathfinding::directed::astar::astar;

/// A position in the routing grid: `(grid_x, grid_y, layer_index)`.
///
/// - `grid_x`, `grid_y`: cell coordinates (0-indexed)
/// - `layer_index`: copper layer (0 = top, 1 = bottom, 2+ = inner)
pub type GridNode = (u16, u16, u8);

/// 8-directional movement offsets: N, NE, E, SE, S, SW, W, NW.
const DIRECTIONS: [(i32, i32); 8] = [
    (0, -1),  // N
    (1, -1),  // NE
    (1, 0),   // E
    (1, 1),   // SE
    (0, 1),   // S
    (-1, 1),  // SW
    (-1, 0),  // W
    (-1, -1), // NW
];

/// An area around a pad endpoint that should be considered passable
/// even if it has obstacle flags set (pads are obstacles to other nets
/// but must be reachable by their own net).
#[derive(Debug, Clone, Copy)]
pub struct PadZone {
    /// Center grid x.
    pub cx: u16,
    /// Center grid y.
    pub cy: u16,
    /// Radius in grid cells.
    pub radius: u16,
}

/// Find a path between two grid nodes using A* search.
///
/// The path traverses the routing grid using 8-directional movement
/// on each layer and via transitions between layers. The cost function
/// penalizes vias, non-preferred layers, and diagonal movement.
///
/// After a path is found, all path cells are marked as occupied on the
/// grid so subsequent routing calls will route around them.
///
/// # Arguments
///
/// * `grid` - The routing grid with occupancy data
/// * `start` - Source pad position as a grid node
/// * `end` - Target pad position as a grid node
/// * `cost` - PCB-aware cost function
/// * `any_end_layer` - If `true`, the path can end on any layer at the
///   goal (x, y) position (for through-hole pads). If `false`, must match
///   the exact layer of `end`.
/// * `pad_zones` - Areas around pad endpoints that should be treated as
///   passable regardless of obstacle flags. This allows routes to enter
///   and exit their own pads which are otherwise marked as obstacles.
///
/// # Returns
///
/// The path as a `Vec<GridNode>` (including start and end), or `None`
/// if no path exists. On success, path cells are marked on the grid.
pub fn find_path(
    grid: &mut RoutingGrid,
    start: GridNode,
    end: GridNode,
    cost: &RoutingCost,
    any_end_layer: bool,
) -> Option<Vec<GridNode>> {
    find_path_with_zones(grid, start, end, cost, any_end_layer, &[])
}

/// Find a path with explicit pad zones that override obstacle checking.
pub fn find_path_with_zones(
    grid: &mut RoutingGrid,
    start: GridNode,
    end: GridNode,
    cost: &RoutingCost,
    any_end_layer: bool,
    pad_zones: &[PadZone],
) -> Option<Vec<GridNode>> {
    let _span = tracing::debug_span!(
        "find_path",
        net_id = cost.net_id(),
        start_x = start.0,
        start_y = start.1,
        start_layer = start.2,
        end_x = end.0,
        end_y = end.1,
        end_layer = end.2,
    )
    .entered();

    let grid_w = grid.width();
    let grid_h = grid.height();
    let layer_count = grid.layer_count();

    // Validate start/end are within grid bounds
    if start.0 as u32 >= grid_w
        || start.1 as u32 >= grid_h
        || start.2 >= layer_count
        || end.0 as u32 >= grid_w
        || end.1 as u32 >= grid_h
        || end.2 >= layer_count
    {
        tracing::warn!(
            grid_w,
            grid_h,
            layer_count,
            "find_path: start or end out of grid bounds"
        );
        return None;
    }

    let net_id = cost.net_id();

    // Success condition: reached goal position (and optionally correct layer)
    let success = |node: &GridNode| -> bool {
        node.0 == end.0 && node.1 == end.1 && (any_end_layer || node.2 == end.2)
    };

    // Successor function: 8 neighbors on same layer + via transitions
    let successors = |node: &GridNode| -> Vec<(GridNode, u64)> {
        let mut neighbors = Vec::with_capacity(10);
        let (nx, ny, nl) = *node;

        // 8-directional movement on the same layer
        for &(dx, dy) in &DIRECTIONS {
            let new_x = nx as i32 + dx;
            let new_y = ny as i32 + dy;

            if new_x < 0 || new_y < 0 {
                continue;
            }
            let ux = new_x as u32;
            let uy = new_y as u32;
            if ux >= grid_w || uy >= grid_h {
                continue;
            }

            // A diagonal step passes between the two cells beside it. If
            // another net owns one of those, the two diagonals cross without
            // ever sharing a cell - copper on copper at 0.00mm.
            //
            // PathFinder's own search has refused that since the crossing
            // violations were traced to it; this search, which the A* strategy
            // uses, never got the same guard. Measured on stm32_breakout:
            // trace-to-trace violations on cells the grid calls free, 17 under
            // PathFinder against 118 under A*.
            if dx != 0 && dy != 0 {
                let side_a = grid.net_at(ux, ny as u32, nl as usize);
                let side_b = grid.net_at(nx as u32, uy, nl as usize);
                let blocked = |owner: Option<u32>| matches!(owner, Some(other) if other != net_id);
                if blocked(side_a) || blocked(side_b) {
                    continue;
                }
            }

            // Check if the target cell is free, is our destination, or is within a pad zone
            let target = (ux as u16, uy as u16, nl);
            if grid.is_free(ux, uy, nl as usize)
                || success(&target)
                || in_pad_zone(ux as u16, uy as u16, pad_zones)
            {
                let c = cost.neighbor_cost(*node, target);
                // Convert f64 cost to u64 with 3 decimal digits of precision
                neighbors.push((target, float_to_int_cost(c)));
            }
        }

        // Via transitions: same (x,y), different layer
        for target_layer in 0..layer_count {
            if target_layer == nl {
                continue;
            }
            let target = (nx, ny, target_layer);
            if grid.is_free(nx as u32, ny as u32, target_layer as usize)
                || success(&target)
                || in_pad_zone(nx, ny, pad_zones)
            {
                let c = cost.neighbor_cost(*node, target);
                neighbors.push((target, float_to_int_cost(c)));
            }
        }

        neighbors
    };

    // Heuristic function (must return u64 matching cost scale)
    let heuristic = |node: &GridNode| -> u64 { float_to_int_cost(cost.heuristic(*node, end)) };

    // Run A*
    let result = astar(&start, successors, heuristic, success);

    match result {
        Some((path, _total_cost)) => {
            let path_len = path.len();
            let via_count = path.windows(2).filter(|w| w[0].2 != w[1].2).count();

            tracing::debug!(path_length = path_len, via_count, "Path found");

            // Mark all path cells as occupied on the grid
            for node in &path {
                grid.mark_route(node.0 as u32, node.1 as u32, node.2 as usize, net_id);
            }

            Some(path)
        }
        None => {
            let stats = grid.stats();
            tracing::warn!(
                start_x = start.0,
                start_y = start.1,
                start_layer = start.2,
                end_x = end.0,
                end_y = end.1,
                end_layer = end.2,
                grid_width = stats.width,
                grid_height = stats.height,
                obstacle_count = stats.obstacle_cell_count,
                "No path found"
            );
            None
        }
    }
}

/// Check if a position is within any of the provided pad zones.
#[inline]
fn in_pad_zone(x: u16, y: u16, zones: &[PadZone]) -> bool {
    for zone in zones {
        let dx = (x as i32 - zone.cx as i32).unsigned_abs();
        let dy = (y as i32 - zone.cy as i32).unsigned_abs();
        let r = zone.radius as u32;
        if dx <= r && dy <= r && dx * dx + dy * dy <= r * r {
            return true;
        }
    }
    false
}

/// Convert an f64 cost to u64 with milli-unit precision.
///
/// This lets us use the `pathfinding` crate's integer-cost A* while
/// preserving enough precision for √2 diagonal costs.
#[inline]
fn float_to_int_cost(f: f64) -> u64 {
    (f * 1000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{make_test_grid, CELL_OBSTACLE};
    use cypcb_core::Nm;
    use cypcb_rules::signal_class::{SignalClass, SignalClassConstraints};
    use cypcb_rules::{DesignConstraints, RoutingRuleSet};

    /// Minimal rules for pathfinder tests.
    struct TestRules {
        base: DesignConstraints,
    }

    impl TestRules {
        fn new() -> Self {
            Self {
                base: DesignConstraints::default(),
            }
        }
    }

    impl RoutingRuleSet for TestRules {
        fn constraints_for_net(&self, _net_id: u32) -> &DesignConstraints {
            &self.base
        }
        fn constraints_for_class(&self, class: SignalClass) -> SignalClassConstraints {
            class.default_constraints()
        }
        fn via_cost(&self, from_layer: u8, to_layer: u8) -> f64 {
            let span = (from_layer as i16 - to_layer as i16).unsigned_abs() as f64;
            span * 2.0
        }
        fn layer_change_cost(&self, layer: u8) -> f64 {
            if layer == 0 {
                0.1
            } else {
                0.5
            }
        }
        fn clearance_between(&self, _net_a: u32, _net_b: u32) -> Nm {
            self.base.min_clearance
        }
    }

    #[test]
    fn route_on_empty_grid() {
        let mut grid = make_test_grid(20, 20, 100_000, 1);
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 1, 1.0, 0.0, 2);

        let path = find_path(&mut grid, (0, 0, 0), (19, 19, 0), &cost, false);
        assert!(path.is_some(), "Should find path on empty grid");

        let path = path.unwrap();
        // Path should start and end at the right places
        assert_eq!(path.first().unwrap(), &(0, 0, 0));
        assert_eq!(path.last().unwrap(), &(19, 19, 0));

        // Diagonal path should be ~20 steps (19 diagonal moves)
        // Reasonable range: 19-30 steps
        assert!(
            path.len() >= 19 && path.len() <= 30,
            "Path length {} seems unreasonable for 20x20 grid",
            path.len()
        );

        // All moves should be adjacent (1 cell max in each direction)
        for w in path.windows(2) {
            let dx = (w[0].0 as i32 - w[1].0 as i32).abs();
            let dy = (w[0].1 as i32 - w[1].1 as i32).abs();
            assert!(
                dx <= 1 && dy <= 1,
                "Non-adjacent move: {:?} -> {:?}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn route_around_l_shaped_obstacle() {
        let mut grid = make_test_grid(20, 20, 100_000, 1);
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 1, 1.0, 0.0, 2);

        // Create L-shaped obstacle blocking the direct path from (2,2) to (17,17)
        // Vertical wall from (10, 0) to (10, 14)
        for y in 0..15u32 {
            grid.mark_obstacle(10, y, 0, 0, CELL_OBSTACLE);
        }
        // Horizontal wall from (10, 14) to (20, 14)
        for x in 10..20u32 {
            grid.mark_obstacle(x, 14, 0, 0, CELL_OBSTACLE);
        }

        let path = find_path(&mut grid, (2, 2, 0), (17, 17, 0), &cost, false);
        assert!(path.is_some(), "Should find path around L-shaped obstacle");

        let path = path.unwrap();
        // Verify no path cell overlaps with obstacle cells
        for node in &path {
            assert!(
                !grid_cell_has_obstacle(&grid, node, CELL_OBSTACLE),
                "Path crosses obstacle at {:?}",
                node
            );
        }
    }

    #[test]
    fn route_with_via_between_layers() {
        let mut grid = make_test_grid(20, 20, 100_000, 2);
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 1, 1.0, 0.0, 2);

        // Block the direct path on layer 0 with a wall
        for x in 0..20u32 {
            grid.mark_obstacle(x, 10, 0, 0, CELL_OBSTACLE);
        }
        // Layer 1 is clear

        // Route from top-left on layer 0 to bottom-right on layer 0
        // Must go through layer 1 to bypass the wall
        let path = find_path(&mut grid, (5, 5, 0), (5, 15, 0), &cost, false);
        assert!(path.is_some(), "Should find path using via to bypass wall");

        let path = path.unwrap();
        // Verify the path includes at least one layer transition
        let via_count = path.windows(2).filter(|w| w[0].2 != w[1].2).count();
        assert!(
            via_count >= 2,
            "Path should have at least 2 layer transitions (down and back up), got {via_count}"
        );

        // Verify start and end layers are correct
        assert_eq!(path.first().unwrap().2, 0, "Should start on layer 0");
        assert_eq!(path.last().unwrap().2, 0, "Should end on layer 0");
    }

    #[test]
    fn route_impossible_returns_none() {
        let mut grid = make_test_grid(10, 10, 100_000, 1);
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 1, 1.0, 0.0, 2);

        // Completely surround the target with obstacles
        let target = (7u32, 7u32);
        for dx in -1i32..=1 {
            for dy in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let x = (target.0 as i32 + dx) as u32;
                let y = (target.1 as i32 + dy) as u32;
                grid.mark_obstacle(x, y, 0, 0, CELL_OBSTACLE);
            }
        }
        // Also block the target cell itself
        grid.mark_obstacle(target.0, target.1, 0, 0, CELL_OBSTACLE);

        let path = find_path(
            &mut grid,
            (0, 0, 0),
            (target.0 as u16, target.1 as u16, 0),
            &cost,
            false,
        );
        assert!(path.is_none(), "Should return None when target is blocked");
    }

    #[test]
    fn path_cells_marked_as_occupied() {
        let mut grid = make_test_grid(10, 10, 100_000, 1);
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 42, 1.0, 0.0, 2);

        let path = find_path(&mut grid, (0, 0, 0), (5, 0, 0), &cost, false);
        assert!(path.is_some());

        let path = path.unwrap();
        // After routing, path cells should no longer be free
        for node in &path {
            assert!(
                !grid.is_free(node.0 as u32, node.1 as u32, node.2 as usize),
                "Path cell {:?} should be marked as occupied",
                node
            );
        }
    }

    #[test]
    fn any_end_layer_mode() {
        let mut grid = make_test_grid(10, 10, 100_000, 2);
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 1, 1.0, 0.0, 2);

        // With any_end_layer = true, path can end on any layer
        let path = find_path(&mut grid, (0, 0, 0), (5, 5, 1), &cost, true);
        assert!(path.is_some());
        let path = path.unwrap();
        let last = path.last().unwrap();
        assert_eq!(last.0, 5);
        assert_eq!(last.1, 5);
        // Layer can be anything (0 or 1)
    }

    /// Helper: check if a grid cell has a specific obstacle flag set.
    /// We check the raw cell value, not is_free(), because is_free checks all flags.
    fn grid_cell_has_obstacle(grid: &RoutingGrid, node: &GridNode, flag: u8) -> bool {
        let cell = grid.cell(node.0 as u32, node.1 as u32, node.2 as usize);
        (cell & flag) != 0
    }
}
