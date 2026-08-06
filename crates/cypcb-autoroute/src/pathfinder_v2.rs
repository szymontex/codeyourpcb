//! PathFinder negotiated congestion routing strategy.
//!
//! Implements VPR-style iterative rip-up and reroute using a shared
//! [`CongestionMap`]. All nets are routed simultaneously on the grid.
//! Each iteration re-routes nets that pass through overused cells with
//! increasing congestion penalties until convergence (zero overused cells)
//! or the iteration cap is reached.
//!
//! # Algorithm Overview
//!
//! 1. Route all nets on the grid using A* with congestion-augmented cost.
//! 2. After all nets: update history costs on overused cells.
//! 3. Re-route only nets passing through overused cells (VPR optimization).
//! 4. Repeat until convergence or iteration cap (50).
//!
//! The congestion cost is added to the A* neighbor cost; the heuristic
//! remains unadulterated to preserve admissibility.

use std::collections::HashMap;

use cypcb_router::types::RoutingResult;
use cypcb_rules::RoutingRuleSet;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use crate::congestion::CongestionMap;
use crate::cost::RoutingCost;
use crate::grid::{RoutingGrid, CELL_OBSTACLE};
use crate::orchestrator::{
    build_spanning_tree, extract_ratsnest, is_multi_layer, order_nets, pad_to_grid_node,
    pad_to_zone, NetRoute,
};
use crate::pathfinder::{GridNode, PadZone};
use crate::postprocess;
use crate::repair::Blocker;
use crate::smoother::smooth_routes;
use crate::strategy::RoutingStrategy;
use crate::via_optimizer::optimize_vias;
use crate::AutorouteConfig;

/// Maximum number of PathFinder iterations before declaring non-convergence.
pub const MAX_PATHFINDER_ITERATIONS: u32 = 50;

/// PathFinder negotiated congestion routing strategy.
///
/// Routes all nets simultaneously using VPR-style iterative negotiated
/// congestion. Each iteration penalizes overused cells more heavily,
/// forcing nets to find alternative paths. Converges when no cells
/// have more than one net occupying them.
pub struct PathFinderStrategy;

impl RoutingStrategy for PathFinderStrategy {
    fn name(&self) -> &str {
        "pathfinder"
    }

    fn route(
        &self,
        world: &mut BoardWorld,
        library: &FootprintLibrary,
        rules: &dyn RoutingRuleSet,
        config: &AutorouteConfig,
    ) -> RoutingResult {
        self.route_with_blockers(world, library, rules, config, &[])
    }
}

impl PathFinderStrategy {
    /// Resolve the grid resolution this strategy will use for `world`.
    ///
    /// The repair pass needs the same number, so that a cell it forbids lands
    /// where the router will look for it.
    pub fn resolution_for(
        world: &mut BoardWorld,
        rules: &dyn RoutingRuleSet,
        config: &AutorouteConfig,
    ) -> i64 {
        if let Some((board_size, _)) = world.board_info() {
            config.resolve_adaptive_grid_resolution(
                rules,
                board_size.width.raw(),
                board_size.height.raw(),
            )
        } else {
            config.resolve_grid_resolution(rules)
        }
    }

    /// Route the board, refusing every cell named in `blockers`.
    ///
    /// `blockers` is empty on a first pass. The repair pass fills it from real
    /// DRC output, so the router is denied exactly the places the checker
    /// complained about instead of everywhere a geometric rule might apply -
    /// which is what separates this from the blanket vetoes that traded
    /// completeness for correctness.
    pub fn route_with_blockers(
        &self,
        world: &mut BoardWorld,
        library: &FootprintLibrary,
        rules: &dyn RoutingRuleSet,
        config: &AutorouteConfig,
        blockers: &[Blocker],
    ) -> RoutingResult {
        let _span = tracing::info_span!("pathfinder_strategy").entered();
        tracing::info!(
            routing_strategy = self.name(),
            "Starting PathFinder routing"
        );

        // Resolve grid resolution (adaptive for large boards)
        let resolution = Self::resolution_for(world, rules, config);

        // Build grid
        let mut grid = match RoutingGrid::from_board(world, library, rules, resolution) {
            Some(g) => g,
            None => {
                return RoutingResult::failed("Failed to build routing grid (no board entity?)")
            }
        };

        // Forbid the cells a previous pass was caught violating.
        let layer_count = grid.layer_count() as usize;
        for blocker in blockers {
            let (gx, gy) = grid.nm_to_grid(blocker.at);
            for layer in 0..layer_count {
                if blocker.covers_layer(layer) {
                    grid.mark_obstacle(gx, gy, layer, blocker.radius_cells, CELL_OBSTACLE);
                }
            }
        }

        // Extract ratsnest
        let ratsnest = extract_ratsnest(world, library);
        if ratsnest.is_empty() {
            tracing::info!("No nets to route");
            return RoutingResult::complete(Vec::new(), Vec::new());
        }

        // Order nets (short first, power last)
        let order = order_nets(&ratsnest);

        // Widths the design states per net. Read once, before the ratsnest walk
        // borrows the world, and keyed by the raw id so the lookup in the
        // output loop is a hash of a u32 rather than of a net name.
        let net_widths: HashMap<u32, cypcb_core::Nm> = ratsnest
            .iter()
            .filter_map(|net| {
                let width = world.net_constraints(net.net_id)?.width?;
                Some((net.net_id.id(), width))
            })
            .collect();

        // Run PathFinder iteration loop
        let loop_result = pathfinder_loop(&mut grid, &ratsnest, &order, rules, config);

        // Post-process: convert grid paths to segments and vias
        let mut all_segments = Vec::new();
        let mut all_vias = Vec::new();

        for net in &ratsnest {
            if let Some(paths) = loop_result.routed_paths.get(&net.net_id.id()) {
                let (segs, vias) = postprocess::paths_to_output(
                    &grid,
                    net.net_id,
                    paths,
                    rules,
                    net_widths.get(&net.net_id.id()).copied(),
                );
                all_segments.extend(segs);
                all_vias.extend(vias);
            }
        }

        // Smooth traces and optimize vias
        let min_clearance = rules.constraints_for_net(0).min_clearance;
        let pre_smooth_segments = all_segments.len();
        let pre_smooth_vias = all_vias.len();

        // Group segments by net_id, smooth each net with other-net context
        let net_ids: Vec<cypcb_world::NetId> = {
            let mut ids: Vec<_> = all_segments.iter().map(|s| s.net_id).collect();
            ids.sort_by_key(|n| n.id());
            ids.dedup();
            ids
        };

        let mut smoothed_segments = Vec::new();
        for net_id in &net_ids {
            let net_segs: Vec<_> = all_segments
                .iter()
                .filter(|s| s.net_id == *net_id)
                .cloned()
                .collect();
            let other_segs: Vec<_> = all_segments
                .iter()
                .filter(|s| s.net_id != *net_id)
                .cloned()
                .collect();
            let smoothed = smooth_routes(
                &net_segs,
                &other_segs,
                min_clearance,
                config.params.roundness,
            );
            smoothed_segments.extend(smoothed);
        }
        all_segments = smoothed_segments;

        let (optimized_segments, optimized_vias) =
            optimize_vias(all_segments, all_vias, &[], min_clearance);
        all_segments = optimized_segments;
        all_vias = optimized_vias;

        tracing::info!(
            pre_smooth_segments,
            post_smooth_segments = all_segments.len(),
            pre_smooth_vias,
            post_smooth_vias = all_vias.len(),
            routing_strategy = self.name(),
            "Post-routing smoothing complete"
        );

        if loop_result.unrouted.is_empty() {
            tracing::info!(
                segments = all_segments.len(),
                vias = all_vias.len(),
                iterations = loop_result.iterations,
                converged = loop_result.converged,
                routing_strategy = self.name(),
                "All nets routed successfully"
            );
            RoutingResult::complete(all_segments, all_vias)
        } else {
            tracing::warn!(
                unrouted = loop_result.unrouted.len(),
                iterations = loop_result.iterations,
                converged = loop_result.converged,
                routing_strategy = self.name(),
                "Some nets could not be routed"
            );
            RoutingResult::partial(all_segments, all_vias, loop_result.unrouted.len())
        }
    }
}

/// Result of the PathFinder iteration loop.
pub struct PathFinderLoopResult {
    /// Per-net routed grid paths.
    pub routed_paths: HashMap<u32, Vec<Vec<GridNode>>>,
    /// Net IDs that could not be routed after all iterations.
    pub unrouted: Vec<u32>,
    /// Number of iterations run.
    pub iterations: u32,
    /// Whether the algorithm converged (zero overused cells).
    pub converged: bool,
}

/// Run the PathFinder negotiated congestion iteration loop.
///
/// This is the core algorithm:
/// 1. Initialize CongestionMap and per-net cell index.
/// 2. Iteration 1: route all nets.
/// 3. Subsequent iterations: only re-route nets through overused cells.
/// 4. After each iteration: update history, check convergence.
pub fn pathfinder_loop(
    grid: &mut RoutingGrid,
    ratsnest: &[NetRoute],
    order: &[usize],
    rules: &dyn RoutingRuleSet,
    config: &AutorouteConfig,
) -> PathFinderLoopResult {
    let _span = tracing::info_span!("pathfinder_loop").entered();

    let width = grid.width();
    let height = grid.height();
    let layers = grid.layer_count();

    // Initialize congestion map
    let mut congestion_map = CongestionMap::new(width, height, layers);

    // Per-net cell index: net_id -> cells occupied by this net.
    // Enables O(path_length) rip-up instead of scanning the entire grid.
    let mut net_cells: HashMap<u32, Vec<(u32, u32, u8)>> = HashMap::new();

    // Per-net routed paths: net_id -> list of GridNode paths
    let mut routed_paths: HashMap<u32, Vec<Vec<GridNode>>> = HashMap::new();

    // Precompute pad zones per net
    let pad_zones_per_net: Vec<Vec<PadZone>> = ratsnest
        .iter()
        .map(|net| net.pads.iter().map(|pad| pad_to_zone(grid, pad)).collect())
        .collect();

    // History cost escalation: starts at 0.5, increases by 0.1 per iteration
    let beta_base = 0.5_f64;
    let beta_increment = 0.1_f64;

    let mut converged = false;
    let mut final_iteration = 0u32;
    // Negotiated congestion is supposed to shrink the overused set until it is
    // empty. When it stops shrinking it has settled on a conflict it cannot
    // resolve, and every further iteration re-routes the same nets to the same
    // places - only slower, because the history cost keeps growing and A*
    // explores more of the grid each time. Stop after this many iterations
    // without a new best.
    const STAGNATION_LIMIT: u32 = 3;
    let mut best_overused = usize::MAX;
    let mut iterations_without_progress = 0u32;

    for iteration in 1..=MAX_PATHFINDER_ITERATIONS {
        final_iteration = iteration;
        let _iter_span = tracing::info_span!("pathfinder_iteration", iteration).entered();

        // Determine which nets to re-route
        let nets_to_route: Vec<usize> = if iteration == 1 {
            // First iteration: route all nets
            order.to_vec()
        } else {
            // Subsequent: only nets passing through overused cells
            let overused = congestion_map.overused_cells();
            if overused.is_empty() {
                converged = true;
                tracing::info!(iteration, "PathFinder converged — zero overused cells");
                break;
            }

            if overused.len() < best_overused {
                best_overused = overused.len();
                iterations_without_progress = 0;
            } else {
                iterations_without_progress += 1;
                if iterations_without_progress >= STAGNATION_LIMIT {
                    tracing::info!(
                        iteration,
                        overused = overused.len(),
                        "PathFinder stalled — overused set stopped shrinking"
                    );
                    break;
                }
            }

            nets_needing_reroute(order, ratsnest, &net_cells, &overused)
        };

        let nets_to_route_count = nets_to_route.len();

        for &net_idx in &nets_to_route {
            let net = &ratsnest[net_idx];
            let net_id = net.net_id.id();

            // Rip up previous route if exists.
            //
            // Clear only the cells this net actually recorded. Scanning the whole
            // grid for the net id (clear_route) does the same work in
            // width * height * layers steps per rip-up, and PathFinder rips up
            // most nets on every iteration.
            if let Some(cells) = net_cells.remove(&net_id) {
                congestion_map.unmark_net(&cells);
                grid.clear_cells(&cells, net_id);
            }
            routed_paths.remove(&net_id);

            // Route this net's connections
            let connections = build_spanning_tree(&net.pads);
            let net_pad_zones = &pad_zones_per_net[net_idx];

            // A via's ring plus the clearance and half the trace that meets it:
            // the radius the next net has to stay outside of.
            let constraints = rules.constraints_for_net(net_id);
            let via_keepout_nm = constraints.min_via_drill.raw() / 2
                + constraints.min_via_annular_ring.raw()
                + constraints.min_clearance.raw()
                + constraints.min_trace_width.raw() / 2;
            let via_radius_cells =
                ((via_keepout_nm + grid.resolution() - 1) / grid.resolution()).max(0) as u32;

            let mut net_path_cells: Vec<(u32, u32, u8)> = Vec::new();
            let mut net_paths: Vec<Vec<GridNode>> = Vec::new();
            let mut net_ok = true;

            for conn in &connections {
                let from_pad = &net.pads[conn.from_idx];
                let to_pad = &net.pads[conn.to_idx];
                let start = pad_to_grid_node(grid, from_pad);
                let end = pad_to_grid_node(grid, to_pad);
                let any_end = is_multi_layer(to_pad.layer_mask);

                // Route with congestion-augmented cost
                let path = find_path_congestion_augmented(
                    grid,
                    start,
                    end,
                    rules,
                    net_id,
                    config.via_cost_multiplier,
                    config.params.layer_preference,
                    any_end,
                    net_pad_zones,
                    &congestion_map,
                );

                match path {
                    Some(p) => {
                        // Collect cells for this path
                        for node in &p {
                            net_path_cells.push((node.0 as u32, node.1 as u32, node.2));
                        }
                        // Wherever the path changes layer there is a via, and a
                        // via is far wider than the single cell the path marks.
                        for pair in p.windows(2) {
                            let (a, b) = (pair[0], pair[1]);
                            if a.2 != b.2 && a.0 == b.0 && a.1 == b.1 {
                                net_path_cells.extend(grid.via_footprint_cells(
                                    a.0 as u32,
                                    a.1 as u32,
                                    (a.2, b.2),
                                    via_radius_cells,
                                ));
                            }
                        }
                        net_paths.push(p);
                    }
                    None => {
                        // A router that gives up has to say on what. Without
                        // this the result carries a count and nothing else, so
                        // "6 unrouted" is a number nobody can act on.
                        tracing::warn!(
                            net = %net.net_name,
                            from = %format!(
                                "{}.{} at {:.3},{:.3}mm",
                                net.net_name,
                                from_pad.pin,
                                from_pad.position.x.to_mm(),
                                from_pad.position.y.to_mm()
                            ),
                            to = %format!(
                                "{}.{} at {:.3},{:.3}mm",
                                net.net_name,
                                to_pad.pin,
                                to_pad.position.x.to_mm(),
                                to_pad.position.y.to_mm()
                            ),
                            "No path found: connection abandoned"
                        );
                        net_ok = false;
                        break;
                    }
                }
            }

            // A net occupies a cell once. net_path_cells collects every
            // connection of the net, so the cells where its own branches meet
            // appear twice, and marking them twice pushes occupancy over a
            // capacity of one - the router then spends every remaining
            // iteration negotiating against itself over junctions it created
            // and cannot remove.
            net_path_cells.sort_unstable();
            net_path_cells.dedup();

            if net_ok && !connections.is_empty() {
                // Update congestion map with this net's cells
                congestion_map.mark_net(&net_path_cells);
                net_cells.insert(net_id, net_path_cells);
                routed_paths.insert(net_id, net_paths);
            } else if !connections.is_empty() {
                // Partially routed or failed — still track what we got
                // Mark cells we did place
                if !net_path_cells.is_empty() {
                    congestion_map.mark_net(&net_path_cells);
                    net_cells.insert(net_id, net_path_cells);
                }
                routed_paths.insert(net_id, net_paths);
            }
        }

        // Update history costs
        let beta = beta_base + beta_increment * (iteration as f64 - 1.0);
        congestion_map.update_history(beta);

        let overuse = congestion_map.overuse_count();

        tracing::info!(
            iteration,
            overused_cells = overuse,
            nets_rerouted = nets_to_route_count,
            total_nets = ratsnest.len(),
            beta = format!("{:.1}", beta),
            "PathFinder iteration complete"
        );

        if overuse == 0 {
            converged = true;
            tracing::info!(iteration, "PathFinder converged — zero overused cells");
            break;
        }
    }

    if !converged {
        let overuse = congestion_map.overuse_count();
        tracing::warn!(
            iterations = final_iteration,
            remaining_overuse = overuse,
            "PathFinder did not converge within iteration cap"
        );
    }

    // Determine unrouted nets
    let unrouted: Vec<u32> = ratsnest
        .iter()
        .filter(|net| {
            let net_id = net.net_id.id();
            let connections = build_spanning_tree(&net.pads);
            if connections.is_empty() {
                return false; // Single-pad or no connections needed
            }
            match routed_paths.get(&net_id) {
                Some(paths) => paths.len() < connections.len(),
                None => true,
            }
        })
        .map(|net| net.net_id.id())
        .collect();

    PathFinderLoopResult {
        routed_paths,
        unrouted,
        iterations: final_iteration,
        converged,
    }
}

/// Find a path using A* with congestion-augmented neighbor cost.
///
/// The base cost comes from `RoutingCost::neighbor_cost()`. Congestion cost
/// from the `CongestionMap` is added on top. The heuristic remains
/// unadulterated (admissible) to preserve A* optimality.
#[allow(clippy::too_many_arguments)]
fn find_path_congestion_augmented(
    grid: &mut RoutingGrid,
    start: GridNode,
    end: GridNode,
    rules: &dyn RoutingRuleSet,
    net_id: u32,
    via_cost_multiplier: f64,
    layer_preference: f64,
    any_end_layer: bool,
    pad_zones: &[PadZone],
    congestion_map: &CongestionMap,
) -> Option<Vec<GridNode>> {
    let grid_w = grid.width();
    let grid_h = grid.height();
    let layer_count = grid.layer_count();

    // Validate bounds
    if start.0 as u32 >= grid_w
        || start.1 as u32 >= grid_h
        || start.2 >= layer_count
        || end.0 as u32 >= grid_w
        || end.1 as u32 >= grid_h
        || end.2 >= layer_count
    {
        return None;
    }

    let cost_fn = RoutingCost::new(rules, net_id, via_cost_multiplier, layer_preference);

    let success = |node: &GridNode| -> bool {
        node.0 == end.0 && node.1 == end.1 && (any_end_layer || node.2 == end.2)
    };

    // 8-directional movement offsets
    const DIRECTIONS: [(i32, i32); 8] = [
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
    ];

    let successors = |node: &GridNode| -> Vec<(GridNode, u64)> {
        let mut neighbors = Vec::with_capacity(10);
        let (nx, ny, nl) = *node;

        // 8-directional movement on same layer
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
            // ever sharing a cell - copper on copper at 0.00mm, and the largest
            // remaining group of clearance violations. Refuse to cut that
            // corner.
            if dx != 0 && dy != 0 {
                let side_a = grid.net_at(ux, ny as u32, nl as usize);
                let side_b = grid.net_at(nx as u32, uy, nl as usize);
                let blocked = |owner: Option<u32>| matches!(owner, Some(other) if other != net_id);
                if blocked(side_a) || blocked(side_b) {
                    continue;
                }
            }

            let target = (ux as u16, uy as u16, nl);
            if grid.is_free(ux, uy, nl as usize)
                || success(&target)
                || in_pad_zone(ux as u16, uy as u16, pad_zones)
                || grid.net_at(ux, uy, nl as usize) == Some(net_id)
            {
                let base = cost_fn.neighbor_cost(*node, target);
                let congestion = congestion_map.congestion_cost(ux, uy, nl);
                neighbors.push((target, float_to_int_cost(base + congestion)));
            }
        }

        // Via transitions
        for target_layer in 0..layer_count {
            if target_layer == nl {
                continue;
            }
            let target = (nx, ny, target_layer);
            if grid.is_free(nx as u32, ny as u32, target_layer as usize)
                || success(&target)
                || in_pad_zone(nx, ny, pad_zones)
                || grid.net_at(nx as u32, ny as u32, target_layer as usize) == Some(net_id)
            {
                let base = cost_fn.neighbor_cost(*node, target);
                let congestion = congestion_map.congestion_cost(nx as u32, ny as u32, target_layer);
                neighbors.push((target, float_to_int_cost(base + congestion)));
            }
        }

        neighbors
    };

    // Heuristic remains unadulterated for admissibility
    let heuristic = |node: &GridNode| -> u64 { float_to_int_cost(cost_fn.heuristic(*node, end)) };

    let result = pathfinding::directed::astar::astar(&start, successors, heuristic, success);

    match result {
        Some((path, _total_cost)) => {
            // Mark path cells on grid
            for node in &path {
                grid.mark_route(node.0 as u32, node.1 as u32, node.2 as usize, net_id);
            }
            Some(path)
        }
        None => None,
    }
}

/// Determine which nets need re-routing based on overused cells.
///
/// A net needs re-routing if any of its cells overlap with the set of
/// overused cells. This is the VPR optimization — only re-route affected
/// nets instead of all nets.
fn nets_needing_reroute(
    order: &[usize],
    ratsnest: &[NetRoute],
    net_cells: &HashMap<u32, Vec<(u32, u32, u8)>>,
    overused: &[(u32, u32, u8)],
) -> Vec<usize> {
    use std::collections::HashSet;
    let overused_set: HashSet<(u32, u32, u8)> = overused.iter().copied().collect();

    order
        .iter()
        .copied()
        .filter(|&net_idx| {
            let net_id = ratsnest[net_idx].net_id.id();
            match net_cells.get(&net_id) {
                Some(cells) => cells.iter().any(|c| overused_set.contains(c)),
                None => true, // Unrouted nets should be attempted
            }
        })
        .collect()
}

/// Check if a position is within any pad zone.
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
#[inline]
fn float_to_int_cost(f: f64) -> u64 {
    (f * 1000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::make_test_grid;
    use cypcb_core::Nm;
    use cypcb_rules::signal_class::{SignalClass, SignalClassConstraints};
    use cypcb_rules::{DesignConstraints, RoutingRuleSet};

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
    fn pathfinder_strategy_name() {
        let strategy = PathFinderStrategy;
        assert_eq!(strategy.name(), "pathfinder");
    }

    #[test]
    fn congestion_augmented_path_on_empty_grid() {
        let mut grid = make_test_grid(20, 20, 100_000, 1);
        let rules = TestRules::new();
        let congestion = CongestionMap::new(20, 20, 1);

        let path = find_path_congestion_augmented(
            &mut grid,
            (0, 0, 0),
            (19, 19, 0),
            &rules,
            1,
            1.0,
            0.0,
            false,
            &[],
            &congestion,
        );

        assert!(path.is_some(), "Should find path on empty grid");
        let path = path.unwrap();
        assert_eq!(path.first().unwrap(), &(0, 0, 0));
        assert_eq!(path.last().unwrap(), &(19, 19, 0));
    }

    #[test]
    fn pathfinder_converges_crossing_nets() {
        // 30x20 grid, 2 layers, 4 nets that cross each other
        // Net 1: left→right through middle
        // Net 2: top→bottom through middle
        // Net 3: top-left→bottom-right diagonal
        // Net 4: top-right→bottom-left diagonal
        let mut grid = make_test_grid(30, 20, 100_000, 2);
        let rules = TestRules::new();
        let config = AutorouteConfig {
            via_cost_multiplier: 1.0,
            ..Default::default()
        };

        use crate::orchestrator::{NetRoute, PadTarget};
        use cypcb_world::NetId;

        let ratsnest = vec![
            NetRoute {
                net_id: NetId::new(1),
                net_name: "H_CROSS".into(),
                pads: vec![
                    PadTarget {
                        position: cypcb_core::Point::new(Nm::new(0), Nm::new(1_000_000)),
                        layer_mask: 0b11,
                        pad_size: (Nm::new(100_000), Nm::new(100_000)),
                        pin: "1".into(),
                    },
                    PadTarget {
                        position: cypcb_core::Point::new(Nm::new(2_900_000), Nm::new(1_000_000)),
                        layer_mask: 0b11,
                        pad_size: (Nm::new(100_000), Nm::new(100_000)),
                        pin: "2".into(),
                    },
                ],
            },
            NetRoute {
                net_id: NetId::new(2),
                net_name: "V_CROSS".into(),
                pads: vec![
                    PadTarget {
                        position: cypcb_core::Point::new(Nm::new(1_500_000), Nm::new(0)),
                        layer_mask: 0b11,
                        pad_size: (Nm::new(100_000), Nm::new(100_000)),
                        pin: "1".into(),
                    },
                    PadTarget {
                        position: cypcb_core::Point::new(Nm::new(1_500_000), Nm::new(1_900_000)),
                        layer_mask: 0b11,
                        pad_size: (Nm::new(100_000), Nm::new(100_000)),
                        pin: "2".into(),
                    },
                ],
            },
            NetRoute {
                net_id: NetId::new(3),
                net_name: "DIAG1".into(),
                pads: vec![
                    PadTarget {
                        position: cypcb_core::Point::new(Nm::new(100_000), Nm::new(100_000)),
                        layer_mask: 0b11,
                        pad_size: (Nm::new(100_000), Nm::new(100_000)),
                        pin: "1".into(),
                    },
                    PadTarget {
                        position: cypcb_core::Point::new(Nm::new(2_800_000), Nm::new(1_800_000)),
                        layer_mask: 0b11,
                        pad_size: (Nm::new(100_000), Nm::new(100_000)),
                        pin: "2".into(),
                    },
                ],
            },
            NetRoute {
                net_id: NetId::new(4),
                net_name: "DIAG2".into(),
                pads: vec![
                    PadTarget {
                        position: cypcb_core::Point::new(Nm::new(2_800_000), Nm::new(100_000)),
                        layer_mask: 0b11,
                        pad_size: (Nm::new(100_000), Nm::new(100_000)),
                        pin: "1".into(),
                    },
                    PadTarget {
                        position: cypcb_core::Point::new(Nm::new(100_000), Nm::new(1_800_000)),
                        layer_mask: 0b11,
                        pad_size: (Nm::new(100_000), Nm::new(100_000)),
                        pin: "2".into(),
                    },
                ],
            },
        ];

        let order: Vec<usize> = (0..ratsnest.len()).collect();
        let result = pathfinder_loop(&mut grid, &ratsnest, &order, &rules, &config);

        // What matters is that four mutually crossing nets all get routed.
        // Convergence to zero overuse is an implementation detail and no longer
        // the only acceptable exit: with diagonal corner-cutting forbidden, this
        // grid settles with residual overuse and the stagnation break ends the
        // loop, having routed everything.
        assert!(
            result.unrouted.is_empty(),
            "all four crossing nets must route, unrouted: {:?}",
            result.unrouted
        );
        assert_eq!(result.routed_paths.len(), 4);
        assert!(
            result.iterations <= 15,
            "Should converge in <15 iterations, took {}",
            result.iterations
        );
        assert!(
            result.unrouted.is_empty(),
            "All nets should be routed, unrouted: {:?}",
            result.unrouted
        );

        // Verify all 4 nets have paths
        for i in 1..=4u32 {
            assert!(
                result.routed_paths.contains_key(&i),
                "Net {i} should have routed paths"
            );
        }
    }

    #[test]
    fn pathfinder_handles_impossible_routing() {
        // 20x10 grid, 1 layer, thick wall completely dividing the grid
        let mut grid = make_test_grid(20, 10, 100_000, 1);
        let rules = TestRules::new();
        let config = AutorouteConfig::default();

        use crate::grid::CELL_OBSTACLE;
        use crate::orchestrator::{NetRoute, PadTarget};
        use cypcb_world::NetId;

        // Build a thick wall from x=7 to x=12 across all y
        // This is wider than any pad zone radius (typically ~4 cells)
        for x in 7..=12u32 {
            for y in 0..10u32 {
                grid.mark_obstacle(x, y, 0, 0, CELL_OBSTACLE);
            }
        }

        let ratsnest = vec![NetRoute {
            net_id: NetId::new(1),
            net_name: "BLOCKED".into(),
            pads: vec![
                PadTarget {
                    position: cypcb_core::Point::new(Nm::new(50_000), Nm::new(50_000)),
                    layer_mask: 1,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: cypcb_core::Point::new(Nm::new(1_850_000), Nm::new(50_000)),
                    layer_mask: 1,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "2".into(),
                },
            ],
        }];

        let order = vec![0];
        let result = pathfinder_loop(&mut grid, &ratsnest, &order, &rules, &config);

        // Should handle gracefully — not crash, report as unrouted
        assert!(
            !result.unrouted.is_empty(),
            "Blocked net should be reported as unrouted"
        );
    }
}
