//! A*-based PCB autorouter for CodeYourPCB.
//!
//! Replaces the FreeRouting JAR dependency with a custom routing engine
//! that produces `RoutingResult` output compatible with the existing
//! `apply_routes()` pipeline.
//!
//! # Workflow
//!
//! 1. Build a [`RoutingGrid`] from a `BoardWorld` and design rules
//! 2. Call [`route_board()`] to run the A* router on all unrouted nets
//! 3. The returned [`RoutingResult`] contains segments and vias ready for `apply_routes()`
//!
//! # Example
//!
//! ```rust,ignore
//! use cypcb_autoroute::{route_board, AutorouteConfig};
//! use cypcb_rules::presets::{RulesPreset, PresetRuleSet};
//!
//! let config = AutorouteConfig::default();
//! let preset = RulesPreset::from_name("jlcpcb").unwrap();
//! let rules = PresetRuleSet::new(preset);
//! let result = route_board(&world, &library, &rules, &config);
//! ```

pub mod cost;
pub mod grid;
pub mod orchestrator;
pub mod pathfinder;
pub mod postprocess;

use cypcb_router::types::RoutingResult;
use cypcb_rules::RoutingRuleSet;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// Configuration for the autorouter.
#[derive(Debug, Clone)]
pub struct AutorouteConfig {
    /// Grid resolution in nanometers. Smaller = more precise but slower.
    /// Default: derived from `min_clearance / 2` (~63µm for JLCPCB).
    /// Set to `None` to auto-derive from rules.
    pub grid_resolution_nm: Option<i64>,

    /// Maximum rip-up-and-retry iterations before giving up on a net.
    pub max_ripup_iterations: u32,

    /// Cost multiplier for placing vias. Higher = fewer vias.
    pub via_cost_multiplier: f64,

    /// Whether to prefer routing on the top layer when possible.
    pub prefer_top_layer: bool,
}

impl Default for AutorouteConfig {
    fn default() -> Self {
        Self {
            grid_resolution_nm: None,
            max_ripup_iterations: 10,
            via_cost_multiplier: 1.0,
            prefer_top_layer: true,
        }
    }
}

impl AutorouteConfig {
    /// Resolve the grid resolution: use explicit value or derive from rules.
    pub fn resolve_grid_resolution(&self, rules: &dyn RoutingRuleSet) -> i64 {
        self.grid_resolution_nm.unwrap_or_else(|| {
            let clearance = rules.constraints_for_net(0).min_clearance;
            // Half the minimum clearance gives good resolution
            (clearance.raw() / 2).max(10_000) // floor at 10µm
        })
    }
}

/// Route all unrouted nets on the board.
///
/// This is the main entry point for the autorouter. It builds a routing grid
/// from the board world and design rules, then attempts to route each net
/// using A* pathfinding with rip-up-and-retry.
///
/// # Arguments
///
/// * `world` - The board world containing components, pads, zones, and existing traces
/// * `library` - Footprint library for pad geometry lookup
/// * `rules` - Design rules providing clearances, widths, and via costs
/// * `config` - Autorouter configuration (grid resolution, iteration limits, etc.)
///
/// # Returns
///
/// A `RoutingResult` containing all generated route segments and vias, or a
/// failure status if routing could not complete.
pub fn route_board(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    rules: &dyn RoutingRuleSet,
    config: &AutorouteConfig,
) -> RoutingResult {
    let _span = tracing::info_span!("route_board").entered();

    let resolution = config.resolve_grid_resolution(rules);

    // Build grid
    let mut grid = match grid::RoutingGrid::from_board(world, library, rules, resolution) {
        Some(g) => g,
        None => return RoutingResult::failed("Failed to build routing grid (no board entity?)"),
    };

    // Extract ratsnest
    let ratsnest = orchestrator::extract_ratsnest(world, library);
    if ratsnest.is_empty() {
        tracing::info!("No nets to route");
        return RoutingResult::complete(Vec::new(), Vec::new());
    }

    // Order nets by priority
    let order = orchestrator::order_nets(&ratsnest);

    // Route all nets
    let loop_result = orchestrator::route_all_nets(&mut grid, &ratsnest, &order, rules, config);

    // Convert grid paths to segments and vias via post-processing
    let mut all_segments = Vec::new();
    let mut all_vias = Vec::new();

    for net in &ratsnest {
        if let Some(paths) = loop_result.routed_paths.get(&net.net_id.id()) {
            let (segs, vias) =
                postprocess::paths_to_output(&grid, net.net_id, paths, rules);
            all_segments.extend(segs);
            all_vias.extend(vias);
        }
    }

    if loop_result.unrouted.is_empty() {
        tracing::info!(
            segments = all_segments.len(),
            vias = all_vias.len(),
            "All nets routed successfully"
        );
        RoutingResult::complete(all_segments, all_vias)
    } else {
        tracing::warn!(
            unrouted = loop_result.unrouted.len(),
            "Some nets could not be routed"
        );
        RoutingResult::partial(all_segments, all_vias, loop_result.unrouted.len())
    }
}
