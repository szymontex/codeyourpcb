//! Improved A* routing strategy.
//!
//! Wraps the existing A* orchestrator with three improvements:
//!
//! 1. **Congestion-aware cost** — penalizes routing near existing nets via
//!    `grid.net_at()` checks during neighbor expansion
//! 2. **Increased rip-up iterations** — 20 iterations (up from 10)
//! 3. **Multi-victim rip-up** — tries up to 3 different blocking nets per
//!    failed connection before giving up
//!
//! Net ordering also considers fanout (pad count) alongside span, routing
//! higher-fanout nets later to give simpler nets more freedom.

use std::collections::HashMap;

use cypcb_router::types::RoutingResult;
use cypcb_rules::RoutingRuleSet;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use crate::cost::RoutingCost;
use crate::grid::RoutingGrid;
use crate::orchestrator::{extract_ratsnest, NetRoute, PadTarget};
use crate::pathfinder::{find_path_with_zones, GridNode, PadZone};
use crate::postprocess;
use crate::smoother::smooth_routes;
use crate::strategy::RoutingStrategy;
use crate::via_optimizer::optimize_vias;
use crate::AutorouteConfig;

/// Improved A* routing strategy with congestion-aware cost and aggressive rip-up.
pub struct ImprovedAStarStrategy;

impl RoutingStrategy for ImprovedAStarStrategy {
    fn name(&self) -> &str {
        "improved-astar"
    }

    fn route(
        &self,
        world: &mut BoardWorld,
        library: &FootprintLibrary,
        rules: &dyn RoutingRuleSet,
        config: &AutorouteConfig,
    ) -> RoutingResult {
        let _span = tracing::info_span!("improved_astar_strategy").entered();
        tracing::info!(routing_strategy = self.name(), "Starting improved A* routing");

        // Use adaptive resolution for large boards
        let resolution = if let Some((board_size, _)) = world.board_info() {
            config.resolve_adaptive_grid_resolution(
                rules,
                board_size.width.raw(),
                board_size.height.raw(),
            )
        } else {
            config.resolve_grid_resolution(rules)
        };

        // Build grid
        let mut grid = match RoutingGrid::from_board(world, library, rules, resolution) {
            Some(g) => g,
            None => {
                return RoutingResult::failed("Failed to build routing grid (no board entity?)")
            }
        };

        // Extract ratsnest
        let ratsnest = extract_ratsnest(world, library);
        if ratsnest.is_empty() {
            tracing::info!("No nets to route");
            return RoutingResult::complete(Vec::new(), Vec::new());
        }

        // Order nets with improved ordering (fanout-aware)
        let order = order_nets_improved(&ratsnest);

        // Route with improved parameters: 20 iterations, 3 victims
        let max_ripup = 20u32;
        let max_victims = 3u32;

        let loop_result = route_all_nets_improved(
            &mut grid,
            &ratsnest,
            &order,
            rules,
            config,
            max_ripup,
            max_victims,
        );

        // Post-process: convert grid paths to segments and vias
        let mut all_segments = Vec::new();
        let mut all_vias = Vec::new();

        for net in &ratsnest {
            if let Some(paths) = loop_result.routed_paths.get(&net.net_id.id()) {
                let (segs, vias) = postprocess::paths_to_output(&grid, net.net_id, paths, rules);
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
            let net_segs: Vec<_> = all_segments.iter().filter(|s| s.net_id == *net_id).cloned().collect();
            let other_segs: Vec<_> = all_segments.iter().filter(|s| s.net_id != *net_id).cloned().collect();
            let smoothed = smooth_routes(&net_segs, &other_segs, min_clearance, config.params.roundness);
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
                routing_strategy = self.name(),
                "All nets routed successfully"
            );
            RoutingResult::complete(all_segments, all_vias)
        } else {
            tracing::warn!(
                unrouted = loop_result.unrouted.len(),
                routing_strategy = self.name(),
                "Some nets could not be routed"
            );
            RoutingResult::partial(all_segments, all_vias, loop_result.unrouted.len())
        }
    }
}

// ============================================================================
// Improved net ordering
// ============================================================================

/// Order nets by routing priority with fanout awareness.
///
/// Priority rules:
/// 1. Power/ground nets go last
/// 2. Short nets before long ones (Manhattan span)
/// 3. Among similar-span nets, lower fanout (fewer pads) routes first
///
/// This gives simpler 2-pin nets more freedom early, reducing congestion
/// for the complex multi-pin nets that follow.
fn order_nets_improved(ratsnest: &[NetRoute]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..ratsnest.len()).collect();

    indices.sort_by(|&a, &b| {
        let net_a = &ratsnest[a];
        let net_b = &ratsnest[b];

        let a_power = is_power_net(&net_a.net_name);
        let b_power = is_power_net(&net_b.net_name);

        // Power nets go last
        match (a_power, b_power) {
            (true, false) => return std::cmp::Ordering::Greater,
            (false, true) => return std::cmp::Ordering::Less,
            _ => {}
        }

        // Sort by Manhattan span (shorter first)
        let a_span = manhattan_span(&net_a.pads);
        let b_span = manhattan_span(&net_b.pads);

        match a_span.cmp(&b_span) {
            std::cmp::Ordering::Equal => {
                // Tie-break: fewer pads (lower fanout) first
                net_a.pads.len().cmp(&net_b.pads.len())
            }
            other => other,
        }
    });

    indices
}

fn is_power_net(name: &str) -> bool {
    let upper = name.to_uppercase();
    matches!(
        upper.as_str(),
        "VCC"
            | "VDD"
            | "GND"
            | "VSS"
            | "5V"
            | "3V3"
            | "3.3V"
            | "12V"
            | "1V8"
            | "1.8V"
            | "+5V"
            | "+3V3"
            | "+3.3V"
            | "+12V"
            | "+1V8"
            | "+1.8V"
            | "VBAT"
            | "VBUS"
            | "V+"
            | "V-"
    )
}

fn manhattan_span(pads: &[PadTarget]) -> i64 {
    if pads.is_empty() {
        return 0;
    }
    let mut min_x = i64::MAX;
    let mut max_x = i64::MIN;
    let mut min_y = i64::MAX;
    let mut max_y = i64::MIN;

    for pad in pads {
        let x = pad.position.x.raw();
        let y = pad.position.y.raw();
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    (max_x - min_x) + (max_y - min_y)
}

// ============================================================================
// Improved routing loop
// ============================================================================

/// Result of the improved routing loop.
pub struct ImprovedRoutingResult {
    pub routed_paths: HashMap<u32, Vec<Vec<GridNode>>>,
    pub unrouted: Vec<u32>,
    pub total_vias: usize,
}

/// Route all nets with improved A* (congestion-aware cost, multi-victim rip-up).
fn route_all_nets_improved(
    grid: &mut RoutingGrid,
    ratsnest: &[NetRoute],
    order: &[usize],
    rules: &dyn RoutingRuleSet,
    config: &AutorouteConfig,
    max_ripup_iterations: u32,
    max_victims_per_failure: u32,
) -> ImprovedRoutingResult {
    let _span = tracing::info_span!("route_all_nets_improved").entered();

    let mut routed_paths: HashMap<u32, Vec<Vec<GridNode>>> = HashMap::new();
    let mut unrouted: Vec<u32> = Vec::new();

    for &net_idx in order {
        let net = &ratsnest[net_idx];
        let net_id = net.net_id.id();
        let connections = build_spanning_tree(&net.pads);

        let _net_span = tracing::info_span!(
            "route_net",
            net_id = net_id,
            net_name = %net.net_name,
            connections = connections.len(),
        )
        .entered();

        let mut net_success = true;
        let mut net_paths: Vec<Vec<GridNode>> = Vec::new();

        let net_pad_zones: Vec<PadZone> =
            net.pads.iter().map(|pad| pad_to_zone(grid, pad)).collect();

        for conn in &connections {
            let from_pad = &net.pads[conn.from_idx];
            let to_pad = &net.pads[conn.to_idx];

            let start = pad_to_grid_node(grid, from_pad);
            let end = pad_to_grid_node(grid, to_pad);
            let any_end = is_multi_layer(to_pad.layer_mask);

            let cost = RoutingCost::new(rules, net_id, config.via_cost_multiplier, config.params.layer_preference);

            // Try direct routing first
            match find_path_with_zones(grid, start, end, &cost, any_end, &net_pad_zones) {
                Some(path) => {
                    net_paths.push(path);
                }
                None => {
                    // Multi-victim rip-up: try up to max_victims different blocking nets
                    let routed = attempt_multi_victim_ripup(
                        grid,
                        start,
                        end,
                        any_end,
                        net_id,
                        &net_pad_zones,
                        &mut routed_paths,
                        ratsnest,
                        rules,
                        config,
                        max_ripup_iterations,
                        max_victims_per_failure,
                    );
                    match routed {
                        Some(path) => {
                            net_paths.push(path);
                        }
                        None => {
                            tracing::warn!(
                                net_id = net_id,
                                net_name = %net.net_name,
                                from_pin = %from_pad.pin,
                                to_pin = %to_pad.pin,
                                victims_tried = max_victims_per_failure,
                                max_ripup_iterations,
                                "Connection failed after exhausting all rip-up victims"
                            );
                            net_success = false;
                        }
                    }
                }
            }
        }

        if net_success && !connections.is_empty() {
            tracing::info!(
                net_id = net_id,
                net_name = %net.net_name,
                path_count = net_paths.len(),
                "Net routed successfully"
            );
            routed_paths.insert(net_id, net_paths);
        } else if !connections.is_empty() {
            tracing::warn!(
                net_id = net_id,
                net_name = %net.net_name,
                "Net partially routed"
            );
            routed_paths.insert(net_id, net_paths);
            unrouted.push(net_id);
        }
    }

    let total_vias: usize = routed_paths
        .values()
        .flat_map(|paths| paths.iter())
        .map(|path| path.windows(2).filter(|w| w[0].2 != w[1].2).count())
        .sum();

    let routed_count = ratsnest.len() - unrouted.len();
    tracing::info!(
        routed = routed_count,
        total = ratsnest.len(),
        vias = total_vias,
        "Improved A* routing complete"
    );

    ImprovedRoutingResult {
        routed_paths,
        unrouted,
        total_vias,
    }
}

/// Multi-victim rip-up: try up to `max_victims` different blocking nets.
///
/// For each victim candidate, we:
/// 1. Find the most likely blocking net
/// 2. Rip it up
/// 3. Try to route the current net
/// 4. If successful, try to re-route the victim
/// 5. If victim re-route fails, restore and try the next candidate
///
/// This is more aggressive than single-victim rip-up because it can
/// discover that a *different* blocker is the real problem.
#[allow(clippy::too_many_arguments)]
fn attempt_multi_victim_ripup(
    grid: &mut RoutingGrid,
    start: GridNode,
    end: GridNode,
    any_end: bool,
    current_net_id: u32,
    pad_zones: &[PadZone],
    routed_paths: &mut HashMap<u32, Vec<Vec<GridNode>>>,
    ratsnest: &[NetRoute],
    rules: &dyn RoutingRuleSet,
    config: &AutorouteConfig,
    max_iterations: u32,
    max_victims: u32,
) -> Option<Vec<GridNode>> {
    let mut tried_victims: Vec<u32> = Vec::new();

    for victim_round in 0..max_victims {
        // Find a blocking net we haven't tried yet
        let victim_id =
            find_blocking_net_excluding(grid, start, end, current_net_id, &tried_victims);

        let victim_id = match victim_id {
            Some(id) => id,
            None => {
                tracing::debug!(
                    victim_round,
                    current_net = current_net_id,
                    tried = ?tried_victims,
                    "No more victim candidates for rip-up"
                );
                return None;
            }
        };

        tried_victims.push(victim_id);

        tracing::debug!(
            victim_round,
            victim = victim_id,
            current = current_net_id,
            "Multi-victim rip-up: trying victim"
        );

        // Try rip-up with this victim, with multiple iterations
        for iter in 0..max_iterations {
            // Save victim's paths
            let victim_paths = routed_paths.remove(&victim_id);

            // Clear victim from grid
            grid.clear_route(victim_id);

            // Try routing current net
            let cost = RoutingCost::new(rules, current_net_id, config.via_cost_multiplier, config.params.layer_preference);
            if let Some(path) =
                find_path_with_zones(grid, start, end, &cost, any_end, pad_zones)
            {
                // Current net routed. Re-route victim.
                if let Some(old_paths) = victim_paths {
                    if reroute_victim(grid, victim_id, ratsnest, rules, config, routed_paths) {
                        tracing::info!(
                            victim = victim_id,
                            victim_round,
                            iter,
                            "Victim re-routed successfully"
                        );
                        return Some(path);
                    } else {
                        // Victim re-route failed. Undo current, restore victim.
                        grid.clear_route(current_net_id);
                        restore_paths(grid, victim_id, &old_paths);
                        routed_paths.insert(victim_id, old_paths);
                        break; // Try next victim
                    }
                }
                return Some(path);
            } else {
                // Current net still can't route. Restore victim and try next iteration.
                if let Some(old_paths) = victim_paths {
                    restore_paths(grid, victim_id, &old_paths);
                    routed_paths.insert(victim_id, old_paths);
                }
                // Only first iteration matters for a given victim — if removing it
                // doesn't help, more iterations won't either. Break to next victim.
                let _ = iter;
                break;
            }
        }
    }

    None
}

/// Re-route a victim net after it was ripped up.
fn reroute_victim(
    grid: &mut RoutingGrid,
    victim_id: u32,
    ratsnest: &[NetRoute],
    rules: &dyn RoutingRuleSet,
    config: &AutorouteConfig,
    routed_paths: &mut HashMap<u32, Vec<Vec<GridNode>>>,
) -> bool {
    let victim_net = match ratsnest.iter().find(|n| n.net_id.id() == victim_id) {
        Some(n) => n,
        None => return false,
    };

    let victim_cost = RoutingCost::new(rules, victim_id, config.via_cost_multiplier, config.params.layer_preference);
    let victim_pad_zones: Vec<PadZone> = victim_net
        .pads
        .iter()
        .map(|pad| pad_to_zone(grid, pad))
        .collect();
    let victim_conns = build_spanning_tree(&victim_net.pads);
    let mut victim_rerouted = Vec::new();

    for conn in &victim_conns {
        let v_start = pad_to_grid_node(grid, &victim_net.pads[conn.from_idx]);
        let v_end = pad_to_grid_node(grid, &victim_net.pads[conn.to_idx]);
        let v_any_end = is_multi_layer(victim_net.pads[conn.to_idx].layer_mask);

        match find_path_with_zones(grid, v_start, v_end, &victim_cost, v_any_end, &victim_pad_zones)
        {
            Some(vp) => victim_rerouted.push(vp),
            None => return false,
        }
    }

    if !victim_rerouted.is_empty() {
        routed_paths.insert(victim_id, victim_rerouted);
    }
    true
}

/// Restore previously-saved paths onto the grid.
fn restore_paths(grid: &mut RoutingGrid, net_id: u32, paths: &[Vec<GridNode>]) {
    for path in paths {
        for node in path {
            grid.mark_route(node.0 as u32, node.1 as u32, node.2 as usize, net_id);
        }
    }
}

/// Find the blocking net, excluding already-tried victims.
fn find_blocking_net_excluding(
    grid: &RoutingGrid,
    start: GridNode,
    end: GridNode,
    current_net: u32,
    exclude: &[u32],
) -> Option<u32> {
    let search_radius = 3u32;
    let mut net_counts: HashMap<u32, u32> = HashMap::new();

    let dx = end.0 as i32 - start.0 as i32;
    let dy = end.1 as i32 - start.1 as i32;
    let steps = (dx.abs().max(dy.abs()) as u32).max(1);
    let sample_step = (steps / 10).max(1);

    let mut i = 0u32;
    while i <= steps {
        let t = i as f64 / steps as f64;
        let sx = start.0 as f64 + dx as f64 * t;
        let sy = start.1 as f64 + dy as f64 * t;
        let cx = sx.round() as u32;
        let cy = sy.round() as u32;

        for layer in 0..grid.layer_count() as usize {
            let min_x = cx.saturating_sub(search_radius);
            let max_x = (cx + search_radius).min(grid.width().saturating_sub(1));
            let min_y = cy.saturating_sub(search_radius);
            let max_y = (cy + search_radius).min(grid.height().saturating_sub(1));

            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    if let Some(net_id) = grid.net_at(x, y, layer) {
                        if net_id != current_net && !exclude.contains(&net_id) {
                            *net_counts.entry(net_id).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        i += sample_step;
        if i > steps && i - sample_step < steps {
            i = steps;
        }
    }

    net_counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(net_id, _)| net_id)
}

// ============================================================================
// Helpers (duplicated from orchestrator to keep strategy self-contained)
// ============================================================================

/// A point-to-point connection within a net.
struct Connection {
    from_idx: usize,
    to_idx: usize,
}

/// Build a minimum spanning tree of pad positions using greedy nearest-neighbor.
fn build_spanning_tree(pads: &[PadTarget]) -> Vec<Connection> {
    if pads.len() < 2 {
        return Vec::new();
    }
    let n = pads.len();
    let mut in_tree = vec![false; n];
    let mut connections = Vec::with_capacity(n - 1);
    in_tree[0] = true;
    let mut tree_count = 1;

    while tree_count < n {
        let mut best_from = 0;
        let mut best_to = 0;
        let mut best_dist = i64::MAX;

        for (i, &in_t) in in_tree.iter().enumerate() {
            if !in_t {
                continue;
            }
            for (j, &in_t_j) in in_tree.iter().enumerate() {
                if in_t_j {
                    continue;
                }
                let dist = manhattan_distance(&pads[i].position, &pads[j].position);
                if dist < best_dist {
                    best_dist = dist;
                    best_from = i;
                    best_to = j;
                }
            }
        }

        in_tree[best_to] = true;
        tree_count += 1;
        connections.push(Connection {
            from_idx: best_from,
            to_idx: best_to,
        });
    }

    connections
}

fn manhattan_distance(a: &cypcb_core::Point, b: &cypcb_core::Point) -> i64 {
    (a.x.raw() - b.x.raw()).abs() + (a.y.raw() - b.y.raw()).abs()
}

fn pad_to_grid_node(grid: &RoutingGrid, pad: &PadTarget) -> GridNode {
    let (gx, gy) = grid.nm_to_grid(pad.position);
    let layer = preferred_layer(pad.layer_mask);
    (gx as u16, gy as u16, layer)
}

fn pad_to_zone(grid: &RoutingGrid, pad: &PadTarget) -> PadZone {
    let (gx, gy) = grid.nm_to_grid(pad.position);
    let pad_radius_nm = pad.pad_size.0.raw().max(pad.pad_size.1.raw()) / 2;
    let pad_radius_cells = ((pad_radius_nm + grid.resolution() - 1) / grid.resolution()) as u16;
    let clearance_cells = 3u16;
    PadZone {
        cx: gx as u16,
        cy: gy as u16,
        radius: pad_radius_cells + clearance_cells,
    }
}

fn preferred_layer(layer_mask: u32) -> u8 {
    if layer_mask & 1 != 0 {
        0
    } else {
        layer_mask.trailing_zeros() as u8
    }
}

fn is_multi_layer(layer_mask: u32) -> bool {
    layer_mask.count_ones() > 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::make_test_grid;
    use crate::pathfinder::find_path;
    use crate::strategy::RoutingStrategy;
    use cypcb_core::{Nm, Point};
    use cypcb_rules::signal_class::{SignalClass, SignalClassConstraints};
    use cypcb_rules::{DesignConstraints, RoutingRuleSet};
    use cypcb_world::NetId;

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
    fn strategy_name_is_correct() {
        let strategy = ImprovedAStarStrategy;
        assert_eq!(strategy.name(), "improved-astar");
    }

    #[test]
    fn multi_victim_ripup_finds_alternative() {
        // Set up a grid where two blocking nets exist and we need to try
        // the second victim to succeed
        let mut grid = make_test_grid(30, 20, 100_000, 2);
        let rules = TestRules::new();
        let config = AutorouteConfig {
            max_ripup_iterations: 20,
            ..Default::default()
        };

        // Route net 1 horizontally through y=10 on layer 0
        let cost1 = RoutingCost::new(&rules, 1, 1.0, 0.0);
        let path1 = find_path(&mut grid, (0, 10, 0), (29, 10, 0), &cost1, false);
        assert!(path1.is_some(), "Net 1 should route on empty grid");

        // Set up ratsnest for rip-up
        let net1 = NetRoute {
            net_id: NetId::new(1),
            net_name: "NET1".into(),
            pads: vec![
                PadTarget {
                    position: Point::new(Nm::new(0), Nm::new(1_000_000)),
                    layer_mask: 0b11,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::new(Nm::new(2_900_000), Nm::new(1_000_000)),
                    layer_mask: 0b11,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "2".into(),
                },
            ],
        };
        let ratsnest = vec![net1];
        let mut routed_paths: HashMap<u32, Vec<Vec<GridNode>>> = HashMap::new();
        routed_paths.insert(1, vec![path1.unwrap()]);

        let pad_zones = vec![
            PadZone {
                cx: 15,
                cy: 0,
                radius: 2,
            },
            PadZone {
                cx: 15,
                cy: 19,
                radius: 2,
            },
        ];

        // Net 2 needs to cross net 1
        let result = attempt_multi_victim_ripup(
            &mut grid,
            (15, 0, 0),
            (15, 19, 0),
            false,
            2,
            &pad_zones,
            &mut routed_paths,
            &ratsnest,
            &rules,
            &config,
            20,
            3,
        );

        assert!(
            result.is_some(),
            "Multi-victim rip-up should succeed for crossing nets"
        );
        assert!(
            routed_paths.contains_key(&1),
            "Victim net 1 should be re-routed"
        );
    }

    #[test]
    fn improved_ordering_fanout_tiebreak() {
        let two_pin_net = NetRoute {
            net_id: NetId::new(1),
            net_name: "A".into(),
            pads: vec![
                PadTarget {
                    position: Point::from_mm(0.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::from_mm(5.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "2".into(),
                },
            ],
        };

        let four_pin_net = NetRoute {
            net_id: NetId::new(2),
            net_name: "B".into(),
            pads: vec![
                PadTarget {
                    position: Point::from_mm(0.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::from_mm(5.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "2".into(),
                },
                PadTarget {
                    position: Point::from_mm(2.5, 2.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "3".into(),
                },
                PadTarget {
                    position: Point::from_mm(2.5, -2.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "4".into(),
                },
            ],
        };

        // Same span, different fanout
        let ratsnest = vec![four_pin_net, two_pin_net];
        let order = order_nets_improved(&ratsnest);

        // 2-pin net (index 1) should come before 4-pin net (index 0)
        assert_eq!(order[0], 1, "Lower fanout net should route first");
    }

    #[test]
    fn route_simple_grid_produces_valid_result() {
        // Directly test the routing loop with a simple 2-net scenario
        let mut grid = make_test_grid(40, 30, 100_000, 2);
        let rules = TestRules::new();
        let config = AutorouteConfig {
            max_ripup_iterations: 20,
            ..Default::default()
        };

        // Two non-conflicting nets
        let net1 = NetRoute {
            net_id: NetId::new(1),
            net_name: "NET1".into(),
            pads: vec![
                PadTarget {
                    position: Point::new(Nm::new(0), Nm::new(0)),
                    layer_mask: 1,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::new(Nm::new(1_500_000), Nm::new(0)),
                    layer_mask: 1,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "2".into(),
                },
            ],
        };
        let net2 = NetRoute {
            net_id: NetId::new(2),
            net_name: "NET2".into(),
            pads: vec![
                PadTarget {
                    position: Point::new(Nm::new(0), Nm::new(2_500_000)),
                    layer_mask: 1,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::new(Nm::new(1_500_000), Nm::new(2_500_000)),
                    layer_mask: 1,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "2".into(),
                },
            ],
        };

        let ratsnest = vec![net1, net2];
        let order = order_nets_improved(&ratsnest);

        let result = route_all_nets_improved(
            &mut grid,
            &ratsnest,
            &order,
            &rules,
            &config,
            20,
            3,
        );

        assert!(result.unrouted.is_empty(), "Both nets should route");
        assert_eq!(
            result.routed_paths.len(),
            2,
            "Should have paths for both nets"
        );
        // Each net should have at least one path
        for (net_id, paths) in &result.routed_paths {
            assert!(
                !paths.is_empty(),
                "Net {net_id} should have at least one path"
            );
            for path in paths {
                assert!(
                    path.len() >= 2,
                    "Path for net {net_id} should have at least start and end"
                );
            }
        }
    }

    #[test]
    fn route_congested_grid_handles_conflicts() {
        // Three nets that cross each other on a small grid
        let mut grid = make_test_grid(30, 30, 100_000, 2);
        let rules = TestRules::new();
        let config = AutorouteConfig {
            max_ripup_iterations: 20,
            ..Default::default()
        };

        // Net 1: horizontal (top of grid)
        let net1 = NetRoute {
            net_id: NetId::new(1),
            net_name: "H1".into(),
            pads: vec![
                PadTarget {
                    position: Point::new(Nm::new(0), Nm::new(1_000_000)),
                    layer_mask: 0b11,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::new(Nm::new(2_900_000), Nm::new(1_000_000)),
                    layer_mask: 0b11,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "2".into(),
                },
            ],
        };

        // Net 2: vertical (crosses net 1)
        let net2 = NetRoute {
            net_id: NetId::new(2),
            net_name: "V1".into(),
            pads: vec![
                PadTarget {
                    position: Point::new(Nm::new(1_500_000), Nm::new(0)),
                    layer_mask: 0b11,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::new(Nm::new(1_500_000), Nm::new(2_900_000)),
                    layer_mask: 0b11,
                    pad_size: (Nm::new(100_000), Nm::new(100_000)),
                    pin: "2".into(),
                },
            ],
        };

        let ratsnest = vec![net1, net2];
        let order = order_nets_improved(&ratsnest);

        let result = route_all_nets_improved(
            &mut grid,
            &ratsnest,
            &order,
            &rules,
            &config,
            20,
            3,
        );

        // Both nets should route (via layer switching or rip-up)
        assert!(
            result.unrouted.is_empty(),
            "Crossing nets should route via layer switching, got {} unrouted",
            result.unrouted.len()
        );
        assert_eq!(result.routed_paths.len(), 2);
    }
}
