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
use crate::grid::{RoutingGrid, CELL_OBSTACLE, CELL_PAD};
use crate::orchestrator::{
    build_spanning_tree, extract_ratsnest, is_multi_layer, order_nets, pad_to_grid_node, NetRoute,
};
use crate::pathfinder::{GridNode, PadZone};
use crate::postprocess;
use crate::repair::Blocker;
use crate::smoother::smooth_routes;
use crate::strategy::RoutingStrategy;
use crate::via_optimizer::optimize_vias;
use crate::AutorouteConfig;

/// What a layer change on a pad's copper costs the search by default.
///
/// Large enough that a route takes any reasonable detour instead, small enough
/// that a net with nowhere else to go still gets through - the alternative is
/// an abandoned connection, and this project has measured five vetoes that
/// cost more than they bought. `AutorouteConfig::pad_layer_change_penalty`
/// carries the value; this is what it defaults to.
pub const PAD_LAYER_CHANGE_PENALTY: f64 = 50.0;

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
        let mut ratsnest = extract_ratsnest(world, library);
        drop_pads_existing_copper_already_joins(world, &mut ratsnest);
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
            if !config.smoothing {
                smoothed_segments
                    .extend(all_segments.iter().filter(|s| s.net_id == *net_id).cloned());
                continue;
            }

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
    /// How many cells were overused at the end of each iteration.
    ///
    /// The trajectory, not just where it stopped. Two runs that end at the
    /// same count can have got there in opposite ways, and the question this
    /// vector is on - why a cost change of a hundredth moves the result by 28
    /// violations - is a question about the path, not the endpoint.
    pub overuse_per_iteration: Vec<usize>,
}

/// Take out the pads a trace already on the board connects.
///
/// The router asks for a spanning tree over every pad of a net, so a net a
/// designer has already wired by hand is routed again and the board ends up
/// with two copies of one connection. A pad sitting on existing copper of its
/// own net is connected; only one pad per piece of copper needs a route to it.
///
/// Approximate on purpose: a pad counts as on a trace when its box overlaps a
/// segment grown by half the trace width. Over-connecting would drop a route
/// that is needed, so the test is the strict one - the pad has to touch.
fn drop_pads_existing_copper_already_joins(world: &mut BoardWorld, ratsnest: &mut [NetRoute]) {
    use cypcb_world::components::trace::{Trace, Via};

    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };
    if traces.is_empty() {
        return;
    }

    let vias: Vec<Via> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        query.iter(ecs).copied().collect()
    };

    for net in ratsnest.iter_mut() {
        // Which of this net's traces each pad sits on.
        let mut on_trace: Vec<Option<usize>> = vec![None; net.pads.len()];
        for (trace_index, trace) in traces.iter().enumerate() {
            if trace.net_id != net.net_id {
                continue;
            }
            let half = trace.width.0 / 2;
            // Copper only connects on the layer it is on. A bottom-layer trace
            // crossing over a top-layer pad is two pieces of copper with the
            // board between them, and treating that as a connection would drop
            // a route the board needs.
            let trace_layer_bit = crate::grid::layer_to_index(trace.layer)
                .filter(|index| *index < 32)
                .map(|index| 1u32 << index);
            for (pad_index, pad) in net.pads.iter().enumerate() {
                if on_trace[pad_index].is_some() {
                    continue;
                }
                match trace_layer_bit {
                    Some(bit) if pad.layer_mask & bit != 0 => {}
                    _ => continue,
                }
                let touches = trace.segments.iter().any(|segment| {
                    let min_x = segment.start.x.0.min(segment.end.x.0) - half;
                    let max_x = segment.start.x.0.max(segment.end.x.0) + half;
                    let min_y = segment.start.y.0.min(segment.end.y.0) - half;
                    let max_y = segment.start.y.0.max(segment.end.y.0) + half;
                    let (half_w, half_h) = (pad.pad_size.0 .0 / 2, pad.pad_size.1 .0 / 2);
                    pad.position.x.0 + half_w >= min_x
                        && pad.position.x.0 - half_w <= max_x
                        && pad.position.y.0 + half_h >= min_y
                        && pad.position.y.0 - half_h <= max_y
                });
                if touches {
                    on_trace[pad_index] = Some(trace_index);
                }
            }
        }

        // A via is copper as well, and one the designer placed joins the
        // traces it lands on into a single piece. Without this, two traces of
        // a net that meet only through a via read as two pieces and the router
        // adds a connection between them that already exists.
        let mut piece: Vec<usize> = (0..traces.len()).collect();
        fn find(piece: &mut [usize], index: usize) -> usize {
            let mut root = index;
            while piece[root] != root {
                root = piece[root];
            }
            let mut walk = index;
            while piece[walk] != root {
                let next = piece[walk];
                piece[walk] = root;
                walk = next;
            }
            root
        }

        for via in &vias {
            if via.net_id != net.net_id {
                continue;
            }
            let reach = via.outer_diameter.0 / 2;
            let mut touched: Vec<usize> = Vec::new();
            for (trace_index, trace) in traces.iter().enumerate() {
                if trace.net_id != net.net_id {
                    continue;
                }
                // Only the layers the via joins: a via reaches its own span
                // and nothing else.
                if !via_reaches_layer(via, trace.layer) {
                    continue;
                }
                let half = trace.width.0 / 2 + reach;
                let lands = trace.segments.iter().any(|segment| {
                    let min_x = segment.start.x.0.min(segment.end.x.0) - half;
                    let max_x = segment.start.x.0.max(segment.end.x.0) + half;
                    let min_y = segment.start.y.0.min(segment.end.y.0) - half;
                    let max_y = segment.start.y.0.max(segment.end.y.0) + half;
                    via.position.x.0 >= min_x
                        && via.position.x.0 <= max_x
                        && via.position.y.0 >= min_y
                        && via.position.y.0 <= max_y
                });
                if lands {
                    touched.push(trace_index);
                }
            }
            for pair in touched.windows(2) {
                let (a, b) = (find(&mut piece, pair[0]), find(&mut piece, pair[1]));
                if a != b {
                    piece[a] = b;
                }
            }
        }

        // One pad per piece of existing copper, plus every pad that touches
        // none.
        let mut kept_traces: Vec<usize> = Vec::new();
        let mut pads = Vec::with_capacity(net.pads.len());
        for (pad_index, pad) in net.pads.iter().enumerate() {
            let group = on_trace[pad_index].map(|index| find(&mut piece, index));
            match group {
                Some(trace_index) if kept_traces.contains(&trace_index) => {
                    tracing::debug!(
                        net = %net.net_name,
                        pin = %pad.pin,
                        "pad already connected by copper on the board"
                    );
                }
                Some(trace_index) => {
                    kept_traces.push(trace_index);
                    pads.push(pad.clone());
                }
                None => pads.push(pad.clone()),
            }
        }
        net.pads = pads;
    }
}

/// Whether a via reaches a given copper layer.
///
/// A through via reaches everything between the faces; a blind or buried one
/// reaches only the layers of its own span.
fn via_reaches_layer(via: &cypcb_world::components::trace::Via, layer: cypcb_world::Layer) -> bool {
    use cypcb_world::Layer;

    let depth = |layer: Layer| -> u16 {
        match layer {
            Layer::TopCopper => 0,
            Layer::Inner(n) => n as u16 + 1,
            _ => u16::MAX,
        }
    };

    let (start, end) = (depth(via.start_layer), depth(via.end_layer));
    let (low, high) = (start.min(end), start.max(end));
    let target = depth(layer);
    target >= low && target <= high
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
    let mut overuse_per_iteration: Vec<usize> = Vec::new();
    congestion_map.set_ring_penalty(config.via_ring_penalty);

    // Per-net cell index: net_id -> cells occupied by this net.
    // Enables O(path_length) rip-up instead of scanning the entire grid.
    let mut net_cells: HashMap<u32, Vec<(u32, u32, u8)>> = HashMap::new();

    // Per-net routed paths: net_id -> list of GridNode paths
    let mut routed_paths: HashMap<u32, Vec<Vec<GridNode>>> = HashMap::new();

    // Precompute pad zones per net
    let pad_zones_per_net: Vec<Vec<PadZone>> = ratsnest
        .iter()
        .map(|net| {
            net.pads
                .iter()
                .map(|pad| {
                    crate::orchestrator::pad_to_zone_with_margin(
                        grid,
                        pad,
                        config.pad_zone_margin_cells,
                    )
                })
                .collect()
        })
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
    let stagnation_limit = config.stagnation_limit;
    // Path cells per net, rings excluded, for reporting contested copper.
    let mut trace_cells: HashMap<u32, Vec<(u32, u32, u8)>> = HashMap::new();
    // Ring cells per net, so a rip-up takes its rings' price with it.
    let mut ring_cells: HashMap<u32, Vec<(u32, u32, u8)>> = HashMap::new();
    let mut best_overused = usize::MAX;
    let mut iterations_without_progress = 0u32;

    // `max_ripup_iterations` has been on `AutorouteConfig` all along and only
    // the legacy orchestrator read it - this strategy ran to a private
    // constant. Zero means the constant, so a caller that says nothing gets
    // what it got before.
    let iteration_cap = if config.max_ripup_iterations == 0 {
        MAX_PATHFINDER_ITERATIONS
    } else {
        config.max_ripup_iterations.min(MAX_PATHFINDER_ITERATIONS)
    };

    for iteration in 1..=iteration_cap {
        final_iteration = iteration;
        let _iter_span = tracing::info_span!("pathfinder_iteration", iteration).entered();

        // Determine which nets to re-route
        let nets_to_route: Vec<usize> = if iteration == 1 {
            // First iteration: route all nets
            order.to_vec()
        } else {
            // Subsequent: only nets passing through overused cells
            let overused = congestion_map.overused_cells();
            overuse_per_iteration.push(overused.len());
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
                if stagnation_limit > 0 && iterations_without_progress >= stagnation_limit {
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
            if let Some(rings) = ring_cells.remove(&net_id) {
                congestion_map.unmark_rings(&rings);
            }
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

            // The copper a via ring actually is, in cells: drill plus annular
            // ring, without the clearance the keepout radius adds. Rounded
            // rather than ceilinged, because a ring 1.09 cells across that is
            // rounded up owns two cells in every direction.
            let via_copper_nm =
                constraints.min_via_drill.raw() / 2 + constraints.min_via_annular_ring.raw();
            let via_copper_cells =
                ((via_copper_nm + grid.resolution() - 1) / grid.resolution()).max(1) as u32;

            let mut net_path_cells: Vec<(u32, u32, u8)> = Vec::new();
            // The path on its own, without the via rings that go into the
            // congestion map with it. Two nets sharing one of these is copper
            // on copper; two nets sharing a ring cell is not, and mixing them
            // is what made the overuse figure unreadable.
            let mut net_trace_cells: Vec<(u32, u32, u8)> = Vec::new();
            let mut net_ring_cells: Vec<(u32, u32, u8)> = Vec::new();
            let mut net_paths: Vec<Vec<GridNode>> = Vec::new();
            let mut net_ok = true;

            for conn in &connections {
                let from_pad = &net.pads[conn.from_idx];
                let to_pad = &net.pads[conn.to_idx];
                let start = pad_to_grid_node(grid, from_pad);
                let end = pad_to_grid_node(grid, to_pad);
                let any_end = is_multi_layer(to_pad.layer_mask);

                // Route with congestion-augmented cost
                let search = Search {
                    rules,
                    net_id,
                    pad_zones: net_pad_zones,
                    via_cost_multiplier: config.via_cost_multiplier,
                    layer_preference: config.params.layer_preference,
                    block_foreign_copper: config.pad_zone_blocks_foreign_copper,
                    via_foreign_copper_penalty: config.via_foreign_copper_penalty,
                    via_foreign_pad_penalty: config.via_foreign_pad_penalty,
                    foreign_pad_penalty: config.foreign_pad_penalty,
                    pad_layer_change_penalty: config.pad_layer_change_penalty,
                    yield_halo: false,
                };
                let mut path = find_path_congestion_augmented(
                    grid,
                    start,
                    end,
                    any_end,
                    &congestion_map,
                    &search,
                );

                // A reservation that cannot be relaxed is a veto, and three
                // measured experiments in this vector say a veto costs more
                // than it buys. When the strict attempt finds nothing, the
                // second one may route through the copper a neighbouring trace
                // only brushes - never through its centre line. An abandoned
                // connection is a board that does not work; a tight gap is a
                // violation the checker will name.
                if path.is_none() && config.reserve_trace_footprint {
                    let relaxed = Search {
                        yield_halo: true,
                        ..search
                    };
                    path = find_path_congestion_augmented(
                        grid,
                        start,
                        end,
                        any_end,
                        &congestion_map,
                        &relaxed,
                    );
                }

                match path {
                    Some(p) => {
                        // Reserve the copper the trace actually covers, not
                        // the centre line the search walked. A minimum-width
                        // trace is 0.127mm on a 0.254mm cell, so the cell next
                        // to a marked one is free as far as the grid knows and
                        // the two nets' copper ends up touching - which is what
                        // 176 of stm32_breakout's 238 clearance violations are.
                        // The returned cells go into net_path_cells so rip-up
                        // clears them; a cell marked and not recorded is a
                        // permanent wall.
                        if config.reserve_trace_footprint {
                            for cell in grid.mark_route_footprint(&p, net_id, 1) {
                                net_path_cells.push(cell);
                                net_trace_cells.push(cell);
                            }
                        } else {
                            for node in &p {
                                net_path_cells.push((node.0 as u32, node.1 as u32, node.2));
                                net_trace_cells.push((node.0 as u32, node.1 as u32, node.2));
                            }
                        }
                        // Wherever the path changes layer there is a via, and a
                        // via is far wider than the single cell the path marks.
                        for pair in p.windows(2) {
                            let (a, b) = (pair[0], pair[1]);
                            if a.2 != b.2 && a.0 == b.0 && a.1 == b.1 {
                                let ring = grid.via_footprint_cells(
                                    a.0 as u32,
                                    a.1 as u32,
                                    (a.2, b.2),
                                    via_radius_cells,
                                );
                                net_ring_cells.extend(ring.iter().copied());
                                net_path_cells.extend(ring);

                                // Under the same flag as the trace above, the
                                // ring's own copper is reserved in the grid.
                                // Not its keepout: marking the full keepout
                                // disc was measured and rejected - at 0.254mm
                                // per cell it blocks 0.508mm around a ring
                                // that is 0.277mm across, and both boards got
                                // worse. This marks what the copper covers,
                                // and only cells no other net already owns.
                                if config.reserve_trace_footprint {
                                    let via = [(a.0, a.1, a.2), (b.0, b.1, b.2)];
                                    for cell in
                                        grid.mark_route_footprint(&via, net_id, via_copper_cells)
                                    {
                                        net_path_cells.push(cell);
                                    }
                                }
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
            net_trace_cells.sort_unstable();
            net_trace_cells.dedup();
            net_ring_cells.sort_unstable();
            net_ring_cells.dedup();

            if net_ok && !connections.is_empty() {
                // Update congestion map with this net's cells
                congestion_map.mark_net(&net_path_cells);
                congestion_map.mark_rings(&net_ring_cells);
                ring_cells.insert(net_id, net_ring_cells);
                trace_cells.insert(net_id, net_trace_cells);
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

        // Which cells two nets' *traces* hold, as opposed to the via rings
        // that make up most of the overuse count. These are shorts: one cell
        // of copper claimed twice.
        let mut holders: HashMap<(u32, u32, u8), Vec<u32>> = HashMap::new();
        for (net_id, cells) in &trace_cells {
            for cell in cells {
                holders.entry(*cell).or_default().push(*net_id);
            }
        }
        let names: HashMap<u32, &str> = ratsnest
            .iter()
            .map(|net| (net.net_id.id(), net.net_name.as_str()))
            .collect();
        let mut contested: Vec<((u32, u32, u8), Vec<u32>)> = holders
            .into_iter()
            .filter(|(_, nets)| nets.len() > 1)
            .collect();
        contested.sort_by_key(|(cell, _)| *cell);

        tracing::warn!(
            iterations = final_iteration,
            remaining_overuse = overuse,
            contested_trace_cells = contested.len(),
            "PathFinder did not converge within iteration cap"
        );

        for (cell, nets) in contested.iter().take(40) {
            let mut held: Vec<&str> = nets
                .iter()
                .map(|id| names.get(id).copied().unwrap_or("?"))
                .collect();
            held.sort_unstable();
            tracing::warn!(
                at = %format!(
                    "cell ({}, {}) layer {} = {:.3},{:.3}mm",
                    cell.0,
                    cell.1,
                    cell.2,
                    grid.grid_to_nm_x(cell.0) as f64 / 1_000_000.0,
                    grid.grid_to_nm_y(cell.1) as f64 / 1_000_000.0
                ),
                nets = %held.join(" + "),
                "Two nets hold one cell of copper"
            );
        }
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
        overuse_per_iteration,
    }
}

/// Find a path using A* with congestion-augmented neighbor cost.
///
/// The base cost comes from `RoutingCost::neighbor_cost()`. Congestion cost
/// from the `CongestionMap` is added on top. The heuristic remains
/// unadulterated (admissible) to preserve A* optimality.
#[allow(clippy::too_many_arguments)]
/// How many cells inside a via's keepout belong to another net.
///
/// Both layers the via joins are counted, because its copper is on both.
fn foreign_cells_in_via_keepout(
    grid: &RoutingGrid,
    x: u32,
    y: u32,
    layers: (u8, u8),
    net_id: u32,
    radius: u32,
) -> (u32, u32) {
    let r = radius as i64;
    let mut routed = 0;
    let mut pads = 0;
    for &layer in &[layers.0, layers.1] {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let cx = x as i64 + dx;
                let cy = y as i64 + dy;
                if cx < 0 || cy < 0 {
                    continue;
                }
                if matches!(
                    grid.net_at(cx as u32, cy as u32, layer as usize),
                    Some(owner) if owner != net_id
                ) {
                    routed += 1;
                } else if matches!(
                    grid.pad_owner(cx as u32, cy as u32, layer as usize),
                    Some(owner) if owner != net_id
                ) {
                    // Counted apart, and priced apart. A pad is copper the
                    // search could not see at all until this existed - a via
                    // paid for landing its ring on another net's trace and
                    // nothing for landing it on another net's pad - and the
                    // two cannot share a price: charging pads the 0.25 a trace
                    // cell costs took stm32_breakout 239 -> 259 and multi_ic
                    // 336 -> 392, because a keepout disc covers many more pad
                    // cells than trace cells on a dense board.
                    pads += 1;
                }
            }
        }
    }
    (routed, pads)
}

/// Everything the search charges for beyond distance, and the net it is
/// charging on behalf of.
///
/// Gathered into one argument because the list had grown to fourteen: each
/// experiment in this vector that survived measurement left a knob behind, and
/// a fourteen-argument call is a place for two of them to be swapped by
/// accident.
struct Search<'a> {
    rules: &'a dyn RoutingRuleSet,
    net_id: u32,
    pad_zones: &'a [PadZone],
    via_cost_multiplier: f64,
    layer_preference: f64,
    block_foreign_copper: bool,
    via_foreign_copper_penalty: f64,
    /// What one cell of another net's **pad** inside a via's keepout costs.
    via_foreign_pad_penalty: f64,
    foreign_pad_penalty: f64,
    /// What a layer change on a pad's copper costs.
    pad_layer_change_penalty: f64,
    /// Whether a net with nowhere else to go may cross reserved copper.
    yield_halo: bool,
}

fn find_path_congestion_augmented(
    grid: &mut RoutingGrid,
    start: GridNode,
    end: GridNode,
    any_end_layer: bool,
    congestion_map: &CongestionMap,
    search: &Search<'_>,
) -> Option<Vec<GridNode>> {
    let Search {
        rules,
        net_id,
        pad_zones,
        via_cost_multiplier,
        layer_preference,
        block_foreign_copper,
        via_foreign_copper_penalty,
        via_foreign_pad_penalty,
        foreign_pad_penalty,
        pad_layer_change_penalty,
        yield_halo,
    } = *search;
    let grid_w = grid.width();
    let grid_h = grid.height();
    let layer_count = grid.layer_count();

    // The radius a via's ring wants clear of foreign copper: the hole, the
    // annular ring, the fab's clearance and half the trace that meets it.
    // Used as a price, never as a veto - see `via_foreign_copper_penalty`.
    let via_keepout_cells = if via_foreign_copper_penalty > 0.0 {
        let constraints = rules.constraints_for_net(net_id);
        let keepout_nm = constraints.min_via_drill.raw() / 2
            + constraints.min_via_annular_ring.raw()
            + constraints.min_clearance.raw()
            + constraints.min_trace_width.raw() / 2;
        ((keepout_nm + grid.resolution() - 1) / grid.resolution()).max(0) as u32
    } else {
        0
    };

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
                let blocked = |owner: Option<u32>, x: u32, y: u32| {
                    if yield_halo && grid.is_halo_only(x, y, nl as usize) {
                        return false;
                    }
                    matches!(owner, Some(other) if other != net_id)
                };
                if blocked(side_a, ux, ny as u32) || blocked(side_b, nx as u32, uy) {
                    continue;
                }
            }

            let target = (ux as u16, uy as u16, nl);
            let free = if yield_halo {
                grid.is_free_ignoring_halo(ux, uy, nl as usize)
            } else {
                grid.is_free(ux, uy, nl as usize)
            };
            if free
                || success(&target)
                || pad_zone_open(
                    grid,
                    ux,
                    uy,
                    nl as usize,
                    net_id,
                    pad_zones,
                    block_foreign_copper,
                )
                || grid.net_at(ux, uy, nl as usize) == Some(net_id)
            {
                let base = cost_fn.neighbor_cost(*node, target);
                let congestion = congestion_map.congestion_cost(ux, uy, nl);

                // Another net's pad copper, priced rather than forbidden. A
                // net's pad zone opens every cell near any of its own pins so
                // a route can reach them, and the pin next door came free with
                // it: 109 of stm32_breakout's 118 part-to-trace faults are
                // routes taking that opening. Refusing it was measured -
                // stm32_breakout lost six connections and multi_ic gained 115
                // violations - which is the seventh veto in this vector to
                // cost more than it buys.
                let foreign_pad = match grid.pad_owner(ux, uy, nl as usize) {
                    Some(owner) if owner != net_id => foreign_pad_penalty,
                    _ => 0.0,
                };
                neighbors.push((target, float_to_int_cost(base + congestion + foreign_pad)));
            }
        }

        // Via transitions
        for target_layer in 0..layer_count {
            if target_layer == nl {
                continue;
            }
            let target = (nx, ny, target_layer);

            // A via may not be placed on a pad's copper or inside its
            // clearance. `paths_to_output` used to delete such vias after the
            // fact, which left the two halves of a route on two layers with
            // nothing joining them - the board came back open and every check
            // agreed it was fine. Refusing the transition here is the same
            // rule applied where it can still be routed around.
            let on_pad = grid.cell(nx as u32, ny as u32, nl as usize) & CELL_PAD != 0
                || grid.cell(nx as u32, ny as u32, target_layer as usize) & CELL_PAD != 0;

            let free = if yield_halo {
                grid.is_free_ignoring_halo(nx as u32, ny as u32, target_layer as usize)
            } else {
                grid.is_free(nx as u32, ny as u32, target_layer as usize)
            };
            if free
                || success(&target)
                || pad_zone_open(
                    grid,
                    nx as u32,
                    ny as u32,
                    target_layer as usize,
                    net_id,
                    pad_zones,
                    block_foreign_copper,
                )
                || grid.net_at(nx as u32, ny as u32, target_layer as usize) == Some(net_id)
            {
                let base = cost_fn.neighbor_cost(*node, target);
                let congestion = congestion_map.congestion_cost(nx as u32, ny as u32, target_layer);
                // A layer change on a pad's copper or inside its clearance is
                // priced, not forbidden. Forbidding it was measured - it moved
                // multi_ic from 140 violations to 375 by pushing the routing
                // somewhere worse - and this vector has five other measurements
                // saying a veto during expansion costs more than it buys.
                let pad_crossing = if on_pad {
                    pad_layer_change_penalty
                } else {
                    0.0
                };
                let crowding = if via_keepout_cells > 0 {
                    let (routed, pads) = foreign_cells_in_via_keepout(
                        grid,
                        nx as u32,
                        ny as u32,
                        (nl, target_layer),
                        net_id,
                        via_keepout_cells,
                    );
                    routed as f64 * via_foreign_copper_penalty
                        + pads as f64 * via_foreign_pad_penalty
                } else {
                    0.0
                };
                neighbors.push((
                    target,
                    float_to_int_cost(base + congestion + crowding + pad_crossing),
                ));
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
/// May this net use this cell on its way into one of its own pads?
///
/// With `block_foreign_copper` off this is the historical behaviour: any cell
/// inside the zone, whatever is already in it. With it on, a cell another
/// net's route holds is refused - the zone is there to get through a keepout,
/// not through someone else's trace.
#[allow(clippy::too_many_arguments)]
fn pad_zone_open(
    grid: &RoutingGrid,
    x: u32,
    y: u32,
    layer: usize,
    net_id: u32,
    zones: &[PadZone],
    block_foreign_copper: bool,
) -> bool {
    if !in_pad_zone(x as u16, y as u16, zones) {
        return false;
    }

    if !block_foreign_copper {
        return true;
    }
    !matches!(grid.net_at(x, y, layer), Some(owner) if owner != net_id)
}

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

        let search = Search {
            rules: &rules,
            net_id: 1,
            pad_zones: &[],
            via_cost_multiplier: 1.0,
            layer_preference: 0.0,
            block_foreign_copper: false,
            via_foreign_copper_penalty: 0.0,
            via_foreign_pad_penalty: 0.0,
            foreign_pad_penalty: 0.0,
            pad_layer_change_penalty: PAD_LAYER_CHANGE_PENALTY,
            yield_halo: false,
        };
        let path = find_path_congestion_augmented(
            &mut grid,
            (0, 0, 0),
            (19, 19, 0),
            false,
            &congestion,
            &search,
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
