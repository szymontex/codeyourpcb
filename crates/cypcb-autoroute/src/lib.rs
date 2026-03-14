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

pub mod astar_improved;
pub mod congestion;
pub mod cost;
pub mod grid;
pub mod orchestrator;
pub mod pathfinder;
pub mod pathfinder_v2;
pub mod postprocess;
pub mod scoring;
pub mod smoother;
pub mod strategy;
pub mod via_optimizer;

use cypcb_router::types::RoutingResult;
use cypcb_rules::RoutingRuleSet;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use strategy::StrategyKind;

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

    /// Which routing strategy to use. Defaults to `StrategyKind::PathFinder`.
    pub strategy: StrategyKind,
}

impl Default for AutorouteConfig {
    fn default() -> Self {
        Self {
            grid_resolution_nm: None,
            max_ripup_iterations: 10,
            via_cost_multiplier: 1.0,
            prefer_top_layer: true,
            strategy: StrategyKind::default(),
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

    /// Resolve grid resolution with adaptive scaling for large boards.
    ///
    /// For boards larger than 80mm in either dimension, the base resolution
    /// is doubled (coarser grid), reducing cell count by 4x and cutting
    /// A* search space proportionally. Quality remains acceptable because
    /// larger boards have proportionally wider trace spacing.
    pub fn resolve_adaptive_grid_resolution(
        &self,
        rules: &dyn RoutingRuleSet,
        board_width_nm: i64,
        board_height_nm: i64,
    ) -> i64 {
        let base = self.resolve_grid_resolution(rules);
        let threshold_nm: i64 = 80_000_000; // 80mm

        if board_width_nm > threshold_nm || board_height_nm > threshold_nm {
            // Scale factor: 2x for boards up to 200mm, 3x for larger
            let max_dim = board_width_nm.max(board_height_nm);
            let scale = if max_dim > 200_000_000 { 3 } else { 2 };
            let adapted = base * scale;
            tracing::info!(
                base_resolution_um = base as f64 / 1_000.0,
                adapted_resolution_um = adapted as f64 / 1_000.0,
                scale,
                board_mm = format!(
                    "{:.0}x{:.0}",
                    board_width_nm as f64 / 1_000_000.0,
                    board_height_nm as f64 / 1_000_000.0
                ),
                "Adaptive grid: coarsening for large board"
            );
            adapted
        } else {
            base
        }
    }
}

/// Route all unrouted nets on the board.
///
/// This is the main entry point for the autorouter. It dispatches to the
/// routing strategy selected in `config.strategy`:
///
/// - `StrategyKind::ImprovedAStar` — congestion-aware A* with multi-victim rip-up
/// - `StrategyKind::PathFinder` — VPR-style negotiated congestion router
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

    let strategy: Box<dyn strategy::RoutingStrategy> = match config.strategy {
        StrategyKind::ImprovedAStar => {
            tracing::info!(strategy = %config.strategy, "Dispatching to ImprovedAStarStrategy");
            Box::new(astar_improved::ImprovedAStarStrategy)
        }
        StrategyKind::PathFinder => {
            tracing::info!(strategy = %config.strategy, "Dispatching to PathFinderStrategy");
            Box::new(pathfinder_v2::PathFinderStrategy)
        }
    };

    strategy.route(world, library, rules, config)
}
