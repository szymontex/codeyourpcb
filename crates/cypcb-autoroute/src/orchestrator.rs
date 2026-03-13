//! Net ordering and routing orchestration.
//!
//! This module is the "brain" of the autorouter. It:
//! 1. Extracts the ratsnest (unrouted connections) from `BoardWorld`
//! 2. Orders nets by routing priority (short first, power last)
//! 3. Routes each net sequentially using A* pathfinding
//! 4. Handles congestion via rip-up/reroute when a net fails

use std::collections::HashMap;

use cypcb_core::{Nm, Point};
use cypcb_rules::RoutingRuleSet;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, FootprintRef, NetConnections, NetId, Position, Rotation};

use crate::cost::RoutingCost;
use crate::grid::{layer_to_index, RoutingGrid};
use crate::pathfinder::{find_path_with_zones, GridNode, PadZone};
use crate::AutorouteConfig;

/// A pad target for routing — the absolute position and layer info of a single pad.
#[derive(Debug, Clone)]
pub struct PadTarget {
    /// Absolute board position in nanometers.
    pub position: Point,
    /// Copper layer mask (bit 0 = top, bit 1 = bottom, ...).
    pub layer_mask: u32,
    /// Pad size (width, height) in nanometers.
    pub pad_size: (Nm, Nm),
    /// Pin name/number for diagnostics.
    pub pin: String,
}

/// A net and its pad targets for routing.
#[derive(Debug, Clone)]
pub struct NetRoute {
    /// The net ID.
    pub net_id: NetId,
    /// The net name (for diagnostics and power/ground detection).
    pub net_name: String,
    /// All pad targets belonging to this net.
    pub pads: Vec<PadTarget>,
}

/// A point-to-point connection within a net (one edge of the spanning tree).
#[derive(Debug, Clone)]
struct Connection {
    from_idx: usize,
    to_idx: usize,
}

/// Extract the ratsnest from a board world.
///
/// For each net, collects all pin pads that belong to it by iterating
/// components and matching pin connections against the footprint library.
pub fn extract_ratsnest(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
) -> Vec<NetRoute> {
    let _span = tracing::info_span!("extract_ratsnest").entered();

    // Collect component data: (position, rotation_deg, footprint_name, net_connections)
    let components: Vec<(Point, f64, String, NetConnections)> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(&Position, &Rotation, &FootprintRef, &NetConnections)>();
        query
            .iter(ecs)
            .map(|(pos, rot, fp, nets)| {
                (pos.0, rot.to_degrees(), fp.as_str().to_string(), nets.clone())
            })
            .collect()
    };

    // Build a map: net_id -> Vec<PadTarget>
    let mut net_pads: HashMap<NetId, Vec<PadTarget>> = HashMap::new();

    for (comp_pos, rotation_deg, fp_name, net_conns) in &components {
        let footprint = match library.get(fp_name) {
            Some(fp) => fp,
            None => {
                tracing::warn!(footprint = %fp_name, "Footprint not found in library, skipping");
                continue;
            }
        };

        for pin_conn in net_conns.iter() {
            // Find the pad definition for this pin
            let pad_def = match footprint.get_pad(&pin_conn.pin) {
                Some(p) => p,
                None => {
                    tracing::debug!(
                        pin = %pin_conn.pin,
                        footprint = %fp_name,
                        "Pin not found in footprint"
                    );
                    continue;
                }
            };

            // Compute absolute pad position
            let pad_pos = rotate_point(pad_def.position, *rotation_deg);
            let abs_pos = Point::new(
                Nm::new(comp_pos.x.raw() + pad_pos.x.raw()),
                Nm::new(comp_pos.y.raw() + pad_pos.y.raw()),
            );

            // Compute layer mask from pad layers
            let mut layer_mask = 0u32;
            for layer in &pad_def.layers {
                if let Some(idx) = layer_to_index(*layer) {
                    layer_mask |= 1u32 << idx;
                }
            }

            net_pads
                .entry(pin_conn.net)
                .or_default()
                .push(PadTarget {
                    position: abs_pos,
                    layer_mask,
                    pad_size: pad_def.size,
                    pin: pin_conn.pin.clone(),
                });
        }
    }

    // Collect net names and build NetRoutes
    let mut ratsnest = Vec::new();
    // Sort by net ID for determinism
    let mut net_ids: Vec<NetId> = net_pads.keys().copied().collect();
    net_ids.sort_by_key(|n| n.id());

    for net_id in net_ids {
        let pads = net_pads.remove(&net_id).unwrap();
        if pads.len() < 2 {
            // Single-pad nets don't need routing (e.g., SIGNAL with one pin)
            continue;
        }

        let net_name = world
            .net_name(net_id)
            .unwrap_or("unnamed")
            .to_string();

        tracing::debug!(
            net_id = net_id.id(),
            net_name = %net_name,
            pad_count = pads.len(),
            "Net extracted"
        );

        ratsnest.push(NetRoute {
            net_id,
            net_name,
            pads,
        });
    }

    tracing::info!(net_count = ratsnest.len(), "Ratsnest extracted");
    ratsnest
}

/// Order nets by routing priority.
///
/// Priority rules:
/// 1. Critical nets (HighSpeed signal class) go first
/// 2. Short nets (smallest Manhattan span) before long ones
/// 3. Power/ground nets go last
///
/// Returns indices into the ratsnest slice, sorted by priority.
pub fn order_nets(ratsnest: &[NetRoute]) -> Vec<usize> {
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
        a_span.cmp(&b_span)
    });

    indices
}

/// Check if a net name indicates power or ground.
fn is_power_net(name: &str) -> bool {
    let upper = name.to_uppercase();
    matches!(
        upper.as_str(),
        "VCC" | "VDD" | "GND" | "VSS" | "5V" | "3V3" | "3.3V" | "12V" | "1V8" | "1.8V"
            | "+5V" | "+3V3" | "+3.3V" | "+12V" | "+1V8" | "+1.8V"
            | "VBAT" | "VBUS" | "V+" | "V-"
    )
}

/// Compute the total Manhattan span of a set of pads (bounding box perimeter proxy).
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

/// Build a minimum spanning tree of pad positions using greedy nearest-neighbor.
///
/// Returns ordered connections (edges) between pad indices.
fn build_spanning_tree(pads: &[PadTarget]) -> Vec<Connection> {
    if pads.len() < 2 {
        return Vec::new();
    }

    let n = pads.len();
    let mut in_tree = vec![false; n];
    let mut connections = Vec::with_capacity(n - 1);

    // Start from pad 0
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

fn manhattan_distance(a: &Point, b: &Point) -> i64 {
    (a.x.raw() - b.x.raw()).abs() + (a.y.raw() - b.y.raw()).abs()
}

/// Result of routing all nets.
pub struct RoutingLoopResult {
    /// Per-net routed grid paths.
    pub routed_paths: HashMap<u32, Vec<Vec<GridNode>>>,
    /// Net IDs that could not be routed.
    pub unrouted: Vec<u32>,
    /// Total via count across all nets.
    pub total_vias: usize,
}

/// Route all nets on the grid with rip-up/reroute.
pub fn route_all_nets(
    grid: &mut RoutingGrid,
    ratsnest: &[NetRoute],
    order: &[usize],
    rules: &dyn RoutingRuleSet,
    config: &AutorouteConfig,
) -> RoutingLoopResult {
    let _span = tracing::info_span!("route_all_nets").entered();

    // Track routed paths per net (net_id -> list of paths)
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

        tracing::info!(
            net_id = net_id,
            net_name = %net.net_name,
            connection_count = connections.len(),
            "Routing net"
        );

        let mut net_success = true;
        let mut net_paths: Vec<Vec<GridNode>> = Vec::new();

        // Compute pad zones for all pads in this net
        let net_pad_zones: Vec<PadZone> = net
            .pads
            .iter()
            .map(|pad| pad_to_zone(grid, pad))
            .collect();

        for conn in &connections {
            let from_pad = &net.pads[conn.from_idx];
            let to_pad = &net.pads[conn.to_idx];

            let start = pad_to_grid_node(grid, from_pad);
            let end = pad_to_grid_node(grid, to_pad);

            // Determine if end pad is through-hole (can end on any layer)
            let any_end = is_multi_layer(to_pad.layer_mask);

            let cost = RoutingCost::new(rules, net_id, config.via_cost_multiplier);

            // Try to route directly
            match find_path_with_zones(grid, start, end, &cost, any_end, &net_pad_zones) {
                Some(path) => {
                    net_paths.push(path);
                }
                None => {
                    // Rip-up/reroute: find blocking net and retry
                    let conn_attempt = ConnectionAttempt {
                        start,
                        end,
                        any_end,
                        net_id,
                        pad_zones: &net_pad_zones,
                    };
                    let routed = attempt_ripup_reroute(
                        grid,
                        &conn_attempt,
                        &mut routed_paths,
                        ratsnest,
                        rules,
                        config,
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
                                "Connection failed after rip-up attempts"
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
            // Partial route — still record what we got
            tracing::warn!(
                net_id = net_id,
                net_name = %net.net_name,
                "Net partially routed"
            );
            routed_paths.insert(net_id, net_paths);
            unrouted.push(net_id);
        }
    }

    // Count total vias
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
        "Routing complete"
    );

    RoutingLoopResult {
        routed_paths,
        unrouted,
        total_vias,
    }
}

/// Parameters for a single connection routing attempt.
struct ConnectionAttempt<'a> {
    start: GridNode,
    end: GridNode,
    any_end: bool,
    net_id: u32,
    pad_zones: &'a [PadZone],
}

/// Attempt rip-up/reroute to resolve congestion.
///
/// Searches around the start/end for blocking nets, rips up the most
/// likely blocker, routes the current net, then re-routes the victim.
#[allow(clippy::too_many_arguments)]
fn attempt_ripup_reroute(
    grid: &mut RoutingGrid,
    attempt: &ConnectionAttempt<'_>,
    routed_paths: &mut HashMap<u32, Vec<Vec<GridNode>>>,
    ratsnest: &[NetRoute],
    rules: &dyn RoutingRuleSet,
    config: &AutorouteConfig,
) -> Option<Vec<GridNode>> {
    let max_iters = config.max_ripup_iterations;
    let current_net_id = attempt.net_id;

    for iter in 0..max_iters {
        // Find a blocking net near the endpoints
        let victim_id = find_blocking_net(grid, attempt.start, attempt.end);

        let victim_id = match victim_id {
            Some(id) if id != current_net_id => id,
            _ => {
                tracing::debug!(
                    iter,
                    current_net = current_net_id,
                    "No suitable victim net found for rip-up"
                );
                return None;
            }
        };

        tracing::warn!(
            iter,
            victim = victim_id,
            current = current_net_id,
            "Rip-up: removing net to make way"
        );

        // Save victim's paths before clearing
        let victim_paths = routed_paths.remove(&victim_id);

        // Clear victim from grid
        grid.clear_route(victim_id);

        // Try routing current net
        let cost = RoutingCost::new(rules, current_net_id, config.via_cost_multiplier);
        if let Some(path) = find_path_with_zones(grid, attempt.start, attempt.end, &cost, attempt.any_end, attempt.pad_zones) {
            // Current net routed. Now re-route victim.
            if let Some(old_paths) = victim_paths {
                let victim_cost = RoutingCost::new(rules, victim_id, config.via_cost_multiplier);
                let mut victim_rerouted = Vec::new();
                let mut victim_ok = true;

                // Find the victim's net route info for pad targets
                let victim_net = ratsnest.iter().find(|n| n.net_id.id() == victim_id);
                if let Some(victim_net) = victim_net {
                    let victim_pad_zones: Vec<PadZone> = victim_net
                        .pads
                        .iter()
                        .map(|pad| pad_to_zone(grid, pad))
                        .collect();
                    let victim_conns = build_spanning_tree(&victim_net.pads);
                    for conn in &victim_conns {
                        let v_start = pad_to_grid_node(grid, &victim_net.pads[conn.from_idx]);
                        let v_end = pad_to_grid_node(grid, &victim_net.pads[conn.to_idx]);
                        let v_any_end =
                            is_multi_layer(victim_net.pads[conn.to_idx].layer_mask);

                        match find_path_with_zones(grid, v_start, v_end, &victim_cost, v_any_end, &victim_pad_zones) {
                            Some(vp) => victim_rerouted.push(vp),
                            None => {
                                victim_ok = false;
                                break;
                            }
                        }
                    }
                }

                if victim_ok {
                    tracing::info!(
                        victim = victim_id,
                        "Victim net re-routed successfully"
                    );
                    if !victim_rerouted.is_empty() {
                        routed_paths.insert(victim_id, victim_rerouted);
                    }
                    return Some(path);
                } else {
                    // Re-routing victim failed. Undo: clear current net, restore victim.
                    tracing::debug!(
                        victim = victim_id,
                        "Victim re-route failed, restoring"
                    );
                    // Clear what we just routed for current net
                    grid.clear_route(current_net_id);
                    // Remove path cells we added
                    // Restore victim's original paths
                    for old_path in &old_paths {
                        for node in old_path {
                            grid.mark_route(
                                node.0 as u32,
                                node.1 as u32,
                                node.2 as usize,
                                victim_id,
                            );
                        }
                    }
                    routed_paths.insert(victim_id, old_paths);
                    // Try next iteration with a different victim
                    continue;
                }
            }

            return Some(path);
        } else {
            // Current net still can't route even with victim removed. Restore victim.
            if let Some(old_paths) = victim_paths {
                for old_path in &old_paths {
                    for node in old_path {
                        grid.mark_route(
                            node.0 as u32,
                            node.1 as u32,
                            node.2 as usize,
                            victim_id,
                        );
                    }
                }
                routed_paths.insert(victim_id, old_paths);
            }
        }
    }

    None
}

/// Find the net that is most likely blocking routing between start and end.
///
/// Samples points along the direct line from start to end and searches
/// around each sample for occupied cells owned by other nets. This finds
/// blockers anywhere along the path, not just near endpoints.
fn find_blocking_net(grid: &RoutingGrid, start: GridNode, end: GridNode) -> Option<u32> {
    let search_radius = 3u32;
    let mut net_counts: HashMap<u32, u32> = HashMap::new();

    // Sample points along the line from start to end
    let dx = end.0 as i32 - start.0 as i32;
    let dy = end.1 as i32 - start.1 as i32;
    let steps = (dx.abs().max(dy.abs()) as u32).max(1);
    // Sample at most ~10 points to keep it fast
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
                        *net_counts.entry(net_id).or_insert(0) += 1;
                    }
                }
            }
        }

        i += sample_step;
        // Always include the last point
        if i > steps && i - sample_step < steps {
            i = steps;
        }
    }

    // Return the net with the most cells along the path
    net_counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(net_id, _)| net_id)
}

/// Convert a PadTarget to the best GridNode for routing.
fn pad_to_grid_node(grid: &RoutingGrid, pad: &PadTarget) -> GridNode {
    let (gx, gy) = grid.nm_to_grid(pad.position);
    // Pick the preferred layer from the layer mask
    let layer = preferred_layer(pad.layer_mask);
    (gx as u16, gy as u16, layer)
}

/// Compute a PadZone (allowed area) for a pad target.
///
/// The zone radius covers the pad's physical extent plus one cell of margin,
/// so that routes can enter and exit the pad even though it's marked as an obstacle.
fn pad_to_zone(grid: &RoutingGrid, pad: &PadTarget) -> PadZone {
    let (gx, gy) = grid.nm_to_grid(pad.position);
    let pad_radius_nm = pad.pad_size.0.raw().max(pad.pad_size.1.raw()) / 2;
    // Pad radius in grid cells, plus clearance bloat, plus 1 cell margin
    let pad_radius_cells =
        ((pad_radius_nm + grid.resolution() - 1) / grid.resolution()) as u16;
    // Add clearance cells (approx same as during grid construction) + margin
    let clearance_cells = 3u16; // Generous but safe
    PadZone {
        cx: gx as u16,
        cy: gy as u16,
        radius: pad_radius_cells + clearance_cells,
    }
}

/// Pick the preferred routing layer from a layer mask.
/// Prefers top copper (layer 0) if available.
fn preferred_layer(layer_mask: u32) -> u8 {
    if layer_mask & 1 != 0 {
        0 // Top copper
    } else {
        // Find lowest set bit
        layer_mask.trailing_zeros() as u8
    }
}

/// Check if a layer mask covers multiple copper layers (through-hole pad).
fn is_multi_layer(layer_mask: u32) -> bool {
    layer_mask.count_ones() > 1
}

/// Rotate a point around the origin by the given angle in degrees.
fn rotate_point(p: Point, degrees: f64) -> Point {
    if degrees.abs() < 0.001 {
        return p;
    }
    let rad = degrees.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let x = p.x.raw() as f64;
    let y = p.y.raw() as f64;
    Point::new(
        Nm::new((x * cos - y * sin).round() as i64),
        Nm::new((x * sin + y * cos).round() as i64),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
            if layer == 0 { 0.1 } else { 0.5 }
        }
        fn clearance_between(&self, _net_a: u32, _net_b: u32) -> Nm {
            self.base.min_clearance
        }
    }

    #[test]
    fn order_nets_short_before_long() {
        let short_net = NetRoute {
            net_id: NetId::new(1),
            net_name: "SHORT".into(),
            pads: vec![
                PadTarget {
                    position: Point::from_mm(0.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::from_mm(1.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "2".into(),
                },
            ],
        };

        let long_net = NetRoute {
            net_id: NetId::new(2),
            net_name: "LONG".into(),
            pads: vec![
                PadTarget {
                    position: Point::from_mm(0.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::from_mm(20.0, 10.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "2".into(),
                },
            ],
        };

        let ratsnest = vec![long_net, short_net];
        let order = order_nets(&ratsnest);

        // Short net (index 1) should come before long net (index 0)
        assert_eq!(order[0], 1, "Short net should be first");
        assert_eq!(order[1], 0, "Long net should be second");
    }

    #[test]
    fn order_nets_power_last() {
        let signal_net = NetRoute {
            net_id: NetId::new(1),
            net_name: "SIGNAL".into(),
            pads: vec![
                PadTarget {
                    position: Point::from_mm(0.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::from_mm(20.0, 20.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "2".into(),
                },
            ],
        };

        let vcc_net = NetRoute {
            net_id: NetId::new(2),
            net_name: "VCC".into(),
            pads: vec![
                PadTarget {
                    position: Point::from_mm(0.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::from_mm(1.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "2".into(),
                },
            ],
        };

        let gnd_net = NetRoute {
            net_id: NetId::new(3),
            net_name: "GND".into(),
            pads: vec![
                PadTarget {
                    position: Point::from_mm(0.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "1".into(),
                },
                PadTarget {
                    position: Point::from_mm(2.0, 0.0),
                    layer_mask: 1,
                    pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                    pin: "2".into(),
                },
            ],
        };

        let ratsnest = vec![vcc_net, signal_net, gnd_net];
        let order = order_nets(&ratsnest);

        // Signal net should come first (non-power), power/ground last
        assert_eq!(order[0], 1, "Signal net should be first");
        // VCC and GND are both power, should be at end
        assert!(
            order[1] == 0 || order[1] == 2,
            "Power nets should be at end"
        );
        assert!(
            order[2] == 0 || order[2] == 2,
            "Power nets should be at end"
        );
    }

    #[test]
    fn ripup_triggers_on_blocked_path() {
        use crate::grid::make_test_grid;
        use crate::pathfinder::find_path;

        // 30x20 grid, 2 layers — net 1 can reroute via layer 1
        let mut grid = make_test_grid(30, 20, 100_000, 2);
        let rules = TestRules::new();
        let config = AutorouteConfig {
            max_ripup_iterations: 3,
            ..Default::default()
        };

        // Route net 1 horizontally through y=10 on layer 0
        let cost1 = RoutingCost::new(&rules, 1, 1.0);
        let path1 = find_path(&mut grid, (0, 10, 0), (29, 10, 0), &cost1, false);
        assert!(path1.is_some(), "Net 1 should route on empty grid");

        // Build a ratsnest with net 1 (so rip-up can re-route it)
        // Pads are through-hole (layer_mask = 0b11) so reroute can use either layer
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

        // Pad zones for net 2 (the current net being routed)
        let net2_pad_zones = vec![
            PadZone { cx: 15, cy: 0, radius: 2 },
            PadZone { cx: 15, cy: 19, radius: 2 },
        ];

        // Net 2 needs to go from (15, 0) to (15, 19) — crosses net 1
        let conn_attempt = ConnectionAttempt {
            start: (15, 0, 0),
            end: (15, 19, 0),
            any_end: false,
            net_id: 2,
            pad_zones: &net2_pad_zones,
        };
        let result = attempt_ripup_reroute(
            &mut grid,
            &conn_attempt,
            &mut routed_paths,
            &ratsnest,
            &rules,
            &config,
        );

        // The rip-up should have succeeded: removed net 1, routed net 2,
        // then re-routed net 1 (can use layer 1 to go around net 2)
        assert!(
            result.is_some(),
            "Rip-up/reroute should succeed for crossing nets"
        );

        // Net 1 should still be in routed_paths (re-routed)
        assert!(
            routed_paths.contains_key(&1),
            "Victim net 1 should be re-routed"
        );
    }

    #[test]
    fn spanning_tree_produces_n_minus_1_edges() {
        let pads: Vec<PadTarget> = (0..5)
            .map(|i| PadTarget {
                position: Point::from_mm(i as f64 * 3.0, 0.0),
                layer_mask: 1,
                pad_size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
                pin: format!("{}", i + 1),
            })
            .collect();

        let tree = build_spanning_tree(&pads);
        assert_eq!(tree.len(), 4, "5 pads should produce 4 edges");
    }

    #[test]
    fn extract_ratsnest_on_test_board() {
        // Build a minimal BoardWorld manually
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();

        // Set up the board
        world.set_board("test".into(), (Nm::from_mm(40.0), Nm::from_mm(25.0)), 2);

        // Intern nets
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        // Component R1 at (10mm, 12mm) with 0402 footprint
        let mut r1_nets = NetConnections::new();
        r1_nets.add(cypcb_world::PinConnection::new("1", vcc));
        r1_nets.add(cypcb_world::PinConnection::new("2", gnd));
        world.spawn_component(
            "R1".into(),
            cypcb_world::Value::new("10k"),
            Position::from_mm(10.0, 12.0),
            Rotation::from_degrees(0.0),
            FootprintRef::new("0402"),
            r1_nets,
        );

        // Component R2 at (25mm, 12mm) with 0402 footprint
        let mut r2_nets = NetConnections::new();
        r2_nets.add(cypcb_world::PinConnection::new("1", gnd));
        r2_nets.add(cypcb_world::PinConnection::new("2", gnd));
        world.spawn_component(
            "R2".into(),
            cypcb_world::Value::new("10k"),
            Position::from_mm(25.0, 12.0),
            Rotation::from_degrees(0.0),
            FootprintRef::new("0402"),
            r2_nets,
        );

        let ratsnest = extract_ratsnest(&mut world, &library);

        // VCC has only 1 pad (R1.1) — below routing threshold, excluded
        // GND has 3 pads (R1.2, R2.1, R2.2) — should appear
        assert_eq!(ratsnest.len(), 1, "Only GND net should have 2+ pads");
        assert_eq!(ratsnest[0].net_name, "GND");
        assert_eq!(ratsnest[0].pads.len(), 3);
    }
}
