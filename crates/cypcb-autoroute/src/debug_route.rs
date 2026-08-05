//! Debug routing: returns intermediate pipeline stages as JSON for visualization.
//!
//! Stages captured:
//! 1. Grid info (dimensions, resolution, blocked cells)
//! 2. Ratsnest (pad pairs to route)
//! 3. Raw grid paths from PathFinder
//! 4. Post-processed segments (grid→nm conversion)
//! 5. Post-smoothing segments
//! 6. Post-via-optimization final output

use crate::grid::RoutingGrid;
use cypcb_rules::RoutingRuleSet;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, NetId};
use serde::Serialize;
use std::collections::HashMap;

use crate::orchestrator::{extract_ratsnest, order_nets};
use crate::pathfinder_v2::pathfinder_loop;
use crate::postprocess;
use crate::smoother::smooth_routes;
use crate::via_optimizer::optimize_vias;
use crate::AutorouteConfig;

/// A single routing stage with serializable segment data.
#[derive(Debug, Clone, Serialize)]
pub struct RoutingStage {
    pub name: String,
    pub segments: Vec<DebugSegment>,
    pub vias: Vec<DebugVia>,
    pub stats: StageStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugSegment {
    pub net_id: u32,
    pub layer: String,
    pub start_x: i64,
    pub start_y: i64,
    pub end_x: i64,
    pub end_y: i64,
    pub width: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugVia {
    pub net_id: u32,
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StageStats {
    pub segment_count: usize,
    pub via_count: usize,
    pub description: String,
}

/// Full debug output from routing pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct RoutingDebugOutput {
    pub grid_width: u32,
    pub grid_height: u32,
    pub grid_resolution_nm: i64,
    pub ratsnest_count: usize,
    pub net_count: usize,
    pub stages: Vec<RoutingStage>,
    pub unrouted_count: usize,
    pub iterations: u32,
    pub converged: bool,
}

fn layer_name(layer: cypcb_world::Layer) -> &'static str {
    match layer {
        cypcb_world::Layer::TopCopper => "top",
        cypcb_world::Layer::BottomCopper => "bottom",
        _ => "other",
    }
}

fn seg_to_debug(seg: &cypcb_router::types::RouteSegment) -> DebugSegment {
    DebugSegment {
        net_id: seg.net_id.id(),
        layer: layer_name(seg.layer).into(),
        start_x: seg.start.x.0,
        start_y: seg.start.y.0,
        end_x: seg.end.x.0,
        end_y: seg.end.y.0,
        width: seg.width.0,
    }
}

fn via_to_debug(via: &cypcb_router::types::ViaPlacement) -> DebugVia {
    DebugVia {
        net_id: via.net_id.id(),
        x: via.position.x.0,
        y: via.position.y.0,
    }
}

/// Run the routing pipeline with full debug output of each stage.
pub fn route_with_debug(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    rules: &dyn RoutingRuleSet,
    config: &AutorouteConfig,
) -> RoutingDebugOutput {
    // Resolve grid resolution
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
            return RoutingDebugOutput {
                grid_width: 0,
                grid_height: 0,
                grid_resolution_nm: resolution,
                ratsnest_count: 0,
                net_count: 0,
                stages: vec![],
                unrouted_count: 0,
                iterations: 0,
                converged: false,
            };
        }
    };

    let grid_width = grid.width();
    let grid_height = grid.height();

    // Extract ratsnest
    let ratsnest = extract_ratsnest(world, library);
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

    let order = order_nets(&ratsnest);

    // Stage 1: PathFinder loop (raw grid paths)
    let loop_result = pathfinder_loop(&mut grid, &ratsnest, &order, rules, config);

    // Stage 2: Post-process (grid paths → segments)
    let mut raw_segments = Vec::new();
    let mut raw_vias = Vec::new();
    for net in &ratsnest {
        if let Some(paths) = loop_result.routed_paths.get(&net.net_id.id()) {
            let (segs, vias) = postprocess::paths_to_output(
                &grid,
                net.net_id,
                paths,
                rules,
                net_widths.get(&net.net_id.id()).copied(),
            );
            raw_segments.extend(segs);
            raw_vias.extend(vias);
        }
    }

    let stage_postprocess = RoutingStage {
        name: "1. Post-process (grid→segments)".into(),
        segments: raw_segments.iter().map(seg_to_debug).collect(),
        vias: raw_vias.iter().map(via_to_debug).collect(),
        stats: StageStats {
            segment_count: raw_segments.len(),
            via_count: raw_vias.len(),
            description: format!(
                "{} segments, {} vias from grid paths",
                raw_segments.len(),
                raw_vias.len()
            ),
        },
    };

    // Stage 3: Smoothing
    let min_clearance = rules.constraints_for_net(0).min_clearance;
    let net_ids: Vec<NetId> = {
        let mut ids: Vec<_> = raw_segments.iter().map(|s| s.net_id).collect();
        ids.sort_by_key(|n| n.id());
        ids.dedup();
        ids
    };

    let mut smoothed_segments = Vec::new();
    for net_id in &net_ids {
        let net_segs: Vec<_> = raw_segments
            .iter()
            .filter(|s| s.net_id == *net_id)
            .cloned()
            .collect();
        let other_segs: Vec<_> = raw_segments
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

    let stage_smooth = RoutingStage {
        name: "2. Smoothing (45°/90° cleanup)".into(),
        segments: smoothed_segments.iter().map(seg_to_debug).collect(),
        vias: raw_vias.iter().map(via_to_debug).collect(),
        stats: StageStats {
            segment_count: smoothed_segments.len(),
            via_count: raw_vias.len(),
            description: format!(
                "{} → {} segments after smoothing",
                raw_segments.len(),
                smoothed_segments.len()
            ),
        },
    };

    // Stage 4: Via optimization
    let (final_segments, final_vias) =
        optimize_vias(smoothed_segments, raw_vias, &[], min_clearance);

    let stage_viaopt = RoutingStage {
        name: "3. Via optimization".into(),
        segments: final_segments.iter().map(seg_to_debug).collect(),
        vias: final_vias.iter().map(via_to_debug).collect(),
        stats: StageStats {
            segment_count: final_segments.len(),
            via_count: final_vias.len(),
            description: format!(
                "Final: {} segments, {} vias",
                final_segments.len(),
                final_vias.len()
            ),
        },
    };

    RoutingDebugOutput {
        grid_width,
        grid_height,
        grid_resolution_nm: resolution,
        ratsnest_count: ratsnest.len(),
        net_count: order.len(),
        stages: vec![stage_postprocess, stage_smooth, stage_viaopt],
        unrouted_count: loop_result.unrouted.len(),
        iterations: loop_result.iterations,
        converged: loop_result.converged,
    }
}
