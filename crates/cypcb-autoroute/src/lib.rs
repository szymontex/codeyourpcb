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
pub mod debug_route;
pub mod grid;
pub mod orchestrator;
pub mod pathfinder;
pub mod pathfinder_v2;
pub mod postprocess;
pub mod repair;
pub mod scoring;
pub mod smoother;
pub mod strategy;
pub mod variant;
pub mod via_optimizer;

use cypcb_router::types::RoutingResult;
use cypcb_rules::RoutingRuleSet;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;
use serde::{Deserialize, Serialize};

use strategy::StrategyKind;

/// User-facing tuning parameters for the autorouter.
///
/// These are the subset of routing settings exposed as sliders in the UI.
/// All fields have sane defaults and can be independently adjusted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutorouteParams {
    /// Via cost multiplier (0.1–10.0, default 1.0). Higher = fewer vias.
    #[serde(default = "default_via_cost")]
    pub via_cost: f64,

    /// Layer preference (-1.0–1.0, default 0.0).
    /// -1.0 = bottom-heavy, 0.0 = balanced, 1.0 = top-heavy.
    #[serde(default)]
    pub layer_preference: f64,

    /// Chamfer roundness (0.0–1.0, default 0.5).
    /// 0.0 = no chamfering, 1.0 = maximum chamfer.
    #[serde(default = "default_roundness")]
    pub roundness: f64,

    /// Grid density multiplier (0.5–2.0, default 1.0).
    /// Higher = finer grid = more precise but slower routing.
    #[serde(default = "default_density")]
    pub density: f64,
}

fn default_via_cost() -> f64 {
    1.0
}
fn default_roundness() -> f64 {
    0.5
}
fn default_density() -> f64 {
    1.0
}

impl Default for AutorouteParams {
    fn default() -> Self {
        Self {
            via_cost: 1.0,
            layer_preference: 0.0,
            roundness: 0.5,
            density: 1.0,
        }
    }
}

impl AutorouteParams {
    /// Return a copy with all fields clamped to their valid ranges.
    pub fn clamped(&self) -> Self {
        Self {
            via_cost: self.via_cost.clamp(0.1, 10.0),
            layer_preference: self.layer_preference.clamp(-1.0, 1.0),
            roundness: self.roundness.clamp(0.0, 1.0),
            density: self.density.clamp(0.5, 2.0),
        }
    }
}

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

    /// User-facing tuning parameters.
    pub params: AutorouteParams,

    /// How many DRC-driven repair passes to run per block radius.
    ///
    /// Each pass forbids the cells the checker complained about and routes
    /// again, keeping the result only if the violation count drops and the
    /// board stays complete. Zero disables repair, which is the default.
    ///
    /// Off by default because it was measured and it does not pay. Every
    /// attempt is a full re-route of the board, and with the default two radii
    /// of two passes that is four of them: multi_ic takes 259.5s with repair
    /// against 74.7s without, and lands on the same 1027 routes and the same
    /// 109 violations. Across all three benchmark fixtures the routed board is
    /// byte-for-byte the same count with the pass and without it. The
    /// machinery is sound and stays available; what it needs before it earns a
    /// place in the default path is an instrument that works in nanometres
    /// rather than in whole grid cells - blocking a 0.254mm cell to fix a
    /// 0.05mm overlap moves the route further than the problem.
    pub repair_passes: u32,

    /// What a cell costs the search for each via ring covering it.
    ///
    /// A via's ring is copper roughly 0.55mm across against a 0.254mm cell,
    /// and the search has never been able to see it: `via_footprint_cells`
    /// feeds the congestion map, where a cell occupied once costs nothing
    /// because the formula charges for overuse rather than presence. So a
    /// route crosses a ring for free. Trace-to-via is the largest group of
    /// 0.00mm overlaps on every benchmark board.
    ///
    /// Zero keeps the old behaviour.
    pub via_ring_penalty: f64,

    /// Whether a route may enter a pad's keepout that another net's copper
    /// already occupies.
    ///
    /// The keepout exists so a route can reach the pad it is heading for. It
    /// admits any cell inside it, occupied or not, which is how two nets end
    /// up on one cell: 20 of stm32_breakout's 26 copper-on-copper overlaps sit
    /// on exactly such a cell. Closing it removes them and costs detours -
    /// 858 segments become 1084 - so it is only worth having if the cost model
    /// can pay for the copper it adds.
    pub pad_zone_blocks_foreign_copper: bool,

    /// Whether to smooth the grid paths into diagonals before emitting them.
    ///
    /// On by default: raw grid output is a staircase, which is longer, uglier
    /// and harder to manufacture than the diagonal it approximates. The switch
    /// exists because the smoother moves copper after the grid has finished
    /// reasoning about clearance, and no measurement could separate the
    /// violations it introduces from the ones it inherits without a run to
    /// compare against.
    pub smoothing: bool,

    /// How many PathFinder iterations may pass without shrinking the overused
    /// set before the loop gives up.
    ///
    /// An overused cell is two nets on one cell of copper, so a run that ends
    /// with overuse ships overlapping traces. The break exists because the
    /// loop used to burn all 50 iterations re-routing the same nets; it is a
    /// trade of correctness for time, and it is worth knowing what each end of
    /// it costs.
    pub stagnation_limit: u32,

    /// Block radii, in cells, that repair tries around each reported violation.
    ///
    /// Each radius is an independent attempt and the best measured result
    /// wins, so this is a list of candidates rather than a tuned constant.
    /// Both are tried because neither wins everywhere. Those numbers were
    /// measured before the clearance rule was fixed; on the current checker
    /// neither radius improves any benchmark board, which is why the pass is
    /// off by default.
    pub repair_block_radii: Vec<u32>,
}

impl Default for AutorouteConfig {
    fn default() -> Self {
        Self {
            grid_resolution_nm: None,
            max_ripup_iterations: 10,
            via_cost_multiplier: 1.0,
            prefer_top_layer: true,
            strategy: StrategyKind::default(),
            params: AutorouteParams::default(),
            via_ring_penalty: 0.0,
            pad_zone_blocks_foreign_copper: false,
            smoothing: true,
            stagnation_limit: 3,
            repair_passes: 0,
            repair_block_radii: vec![0, 2],
        }
    }
}

impl AutorouteConfig {
    /// Resolve the grid resolution: use explicit value or derive from rules.
    pub fn resolve_grid_resolution(&self, rules: &dyn RoutingRuleSet) -> i64 {
        self.grid_resolution_nm.unwrap_or_else(|| {
            // One cell per legal track position: a trace plus the clearance it
            // needs. Neighbouring cells are then clearance-legal by
            // construction, which is what a routing grid is for.
            //
            // The old half-a-clearance grid let two nets sit in adjacent cells
            // whose copper physically overlapped. Measured on stm32_breakout:
            // 238 DRC violations in 127.8s at clearance/2, against 124 in 9.7s
            // at track pitch, with the same board fully routed.
            let constraints = rules.constraints_for_net(0);
            let pitch = constraints.min_trace_width.raw() + constraints.min_clearance.raw();
            pitch.max(10_000) // floor at 10µm
        })
    }

    /// Resolve grid resolution with adaptive scaling for large boards.
    ///
    /// For boards larger than 80mm in either dimension, the base resolution
    /// is doubled (coarser grid), reducing cell count by 4x and cutting
    /// A* search space proportionally. Quality remains acceptable because
    /// larger boards have proportionally wider trace spacing.
    ///
    /// The `params.density` multiplier is applied as the final step:
    /// density > 1.0 → finer grid (smaller resolution), density < 1.0 → coarser.
    /// Resolution is clamped so it never goes below 10µm.
    pub fn resolve_adaptive_grid_resolution(
        &self,
        rules: &dyn RoutingRuleSet,
        board_width_nm: i64,
        board_height_nm: i64,
    ) -> i64 {
        // An explicit resolution is an instruction, not a hint. Scaling it
        // because the board is large meant a caller asking for 0.254mm on a
        // 100mm board silently got 0.508mm, and a benchmark sweep comparing
        // resolutions compared the same grid twice without saying so.
        if let Some(explicit) = self.grid_resolution_nm {
            return explicit.max(10_000);
        }

        let base = self.resolve_grid_resolution(rules);
        let threshold_nm: i64 = 80_000_000; // 80mm

        let adapted = if board_width_nm > threshold_nm || board_height_nm > threshold_nm {
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
        };

        // Apply density multiplier: higher density = finer grid = smaller resolution
        let density = self.params.density.clamp(0.5, 2.0);
        let density_adjusted = (adapted as f64 / density).round() as i64;

        // Floor at 10µm to prevent astronomically large grids
        density_adjusted.max(10_000)
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

    // Apply params.via_cost to via_cost_multiplier if params differ from default
    let mut effective_config = config.clone();
    let clamped = config.params.clamped();
    effective_config.via_cost_multiplier = clamped.via_cost;
    effective_config.params = clamped;

    let strategy: Box<dyn strategy::RoutingStrategy> = match effective_config.strategy {
        StrategyKind::ImprovedAStar => {
            tracing::info!(strategy = %effective_config.strategy, "Dispatching to ImprovedAStarStrategy");
            Box::new(astar_improved::ImprovedAStarStrategy)
        }
        StrategyKind::PathFinder => {
            tracing::info!(strategy = %effective_config.strategy, "Dispatching to PathFinderStrategy");
            Box::new(pathfinder_v2::PathFinderStrategy)
        }
    };

    let result = strategy.route(world, library, rules, &effective_config);

    // Repair re-routes with PathFinder, so it only applies to a PathFinder
    // solution - handing it an A* result would silently swap strategies.
    match effective_config.strategy {
        StrategyKind::PathFinder => {
            repair::repair_routes(world, library, rules, &effective_config, result)
        }
        StrategyKind::ImprovedAStar => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_default_values() {
        let params = AutorouteParams::default();
        assert_eq!(params.via_cost, 1.0);
        assert_eq!(params.layer_preference, 0.0);
        assert_eq!(params.roundness, 0.5);
        assert_eq!(params.density, 1.0);
    }

    #[test]
    fn params_clamped() {
        let params = AutorouteParams {
            via_cost: 100.0,
            layer_preference: -5.0,
            roundness: 2.0,
            density: 0.0,
        };
        let clamped = params.clamped();
        assert_eq!(clamped.via_cost, 10.0);
        assert_eq!(clamped.layer_preference, -1.0);
        assert_eq!(clamped.roundness, 1.0);
        assert_eq!(clamped.density, 0.5);

        // Also check lower bounds
        let params2 = AutorouteParams {
            via_cost: -1.0,
            layer_preference: 0.5,
            roundness: -0.5,
            density: 3.0,
        };
        let clamped2 = params2.clamped();
        assert_eq!(clamped2.via_cost, 0.1);
        assert_eq!(clamped2.layer_preference, 0.5);
        assert_eq!(clamped2.roundness, 0.0);
        assert_eq!(clamped2.density, 2.0);
    }

    #[test]
    fn params_serde_roundtrip() {
        let params = AutorouteParams {
            via_cost: 3.5,
            layer_preference: -0.7,
            roundness: 0.8,
            density: 1.5,
        };
        let json = serde_json::to_string(&params).unwrap();
        let deserialized: AutorouteParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn params_from_json_partial() {
        // Only via_cost specified; other fields should use defaults
        let json = r#"{"via_cost": 5.0}"#;
        let params: AutorouteParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.via_cost, 5.0);
        assert_eq!(params.layer_preference, 0.0);
        assert_eq!(params.roundness, 0.5);
        assert_eq!(params.density, 1.0);
    }

    #[test]
    fn params_from_json_empty() {
        // Empty JSON object should give all defaults
        let json = "{}";
        let params: AutorouteParams = serde_json::from_str(json).unwrap();
        assert_eq!(params, AutorouteParams::default());
    }

    #[test]
    fn params_from_json_invalid() {
        // Malformed JSON should fail to parse
        let json = "not json at all";
        let result: Result<AutorouteParams, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn config_default_has_default_params() {
        let config = AutorouteConfig::default();
        assert_eq!(config.params, AutorouteParams::default());
    }

    #[test]
    fn density_affects_grid_resolution() {
        use cypcb_core::Nm;
        use cypcb_rules::signal_class::{SignalClass, SignalClassConstraints};
        use cypcb_rules::{DesignConstraints, RoutingRuleSet};

        struct TestRules;
        impl RoutingRuleSet for TestRules {
            fn constraints_for_net(&self, _: u32) -> &DesignConstraints {
                // Use a static to satisfy the lifetime
                static CONSTRAINTS: std::sync::OnceLock<DesignConstraints> =
                    std::sync::OnceLock::new();
                CONSTRAINTS.get_or_init(DesignConstraints::default)
            }
            fn constraints_for_class(&self, c: SignalClass) -> SignalClassConstraints {
                c.default_constraints()
            }
            fn via_cost(&self, _: u8, _: u8) -> f64 {
                2.0
            }
            fn layer_change_cost(&self, _: u8) -> f64 {
                0.1
            }
            fn clearance_between(&self, _: u32, _: u32) -> Nm {
                Nm(127_000) // 0.127mm
            }
        }

        let rules = TestRules;
        let mut config = AutorouteConfig::default();

        // Default density = 1.0
        config.params.density = 1.0;
        let res_default = config.resolve_adaptive_grid_resolution(&rules, 50_000_000, 30_000_000);

        // Higher density = finer grid = smaller resolution
        config.params.density = 2.0;
        let res_dense = config.resolve_adaptive_grid_resolution(&rules, 50_000_000, 30_000_000);

        // Lower density = coarser grid = larger resolution
        config.params.density = 0.5;
        let res_coarse = config.resolve_adaptive_grid_resolution(&rules, 50_000_000, 30_000_000);

        assert!(
            res_dense < res_default,
            "density=2.0 should produce finer grid (smaller resolution) than default"
        );
        assert!(
            res_coarse > res_default,
            "density=0.5 should produce coarser grid (larger resolution) than default"
        );
        assert!(res_dense >= 10_000, "resolution should never go below 10µm");
    }
}
