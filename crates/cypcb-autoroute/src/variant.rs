//! Multi-variant routing generation.
//!
//! Generates multiple routing variants with different strategy/param configurations,
//! scores each one, and returns ranked results. The best variant is auto-applied
//! to the world after generation.
//!
//! # Critical constraint
//!
//! `BoardWorld` wraps bevy_ecs `World` which does NOT implement Clone.
//! Variants must be generated sequentially: route → apply → rebuild spatial
//! index → score → serialize route/via data → clear → next variant.

use serde::Serialize;

use cypcb_drc::DesignRules;
use cypcb_router::apply_routes;
use cypcb_router::types::{RouteSegment, RoutingResult, RoutingStatus, ViaPlacement};
use cypcb_rules::RoutingRuleSet;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use crate::scoring::{score_board, RoutingScore, ScoreWeights};
use crate::strategy::StrategyKind;
use crate::{route_board, AutorouteConfig, AutorouteParams};

/// Configuration for a single routing variant.
#[derive(Debug, Clone)]
pub struct VariantConfig {
    /// Human-readable name for this variant.
    pub name: String,
    /// Which routing strategy to use.
    pub strategy: StrategyKind,
    /// Tuning parameters for this variant.
    pub params: AutorouteParams,
    /// What a via ring costs the search, per ring covering a cell.
    pub via_ring_penalty: f64,
    /// Whether a route may cross another net's copper inside a pad keepout.
    pub pad_zone_blocks_foreign_copper: bool,
    /// Whether a routed trace reserves the copper it covers, not just its
    /// centre line.
    pub reserve_trace_footprint: bool,
    /// What crossing another net's pad copper costs, per cell.
    pub foreign_pad_penalty: f64,

    /// How many cells beyond a pad's copper its zone opens.
    pub pad_zone_margin_cells: u16,
    /// What this variant multiplies the search's distance estimate by.
    ///
    /// 1.0 is the admissible heuristic that makes A* return the cheapest path.
    pub heuristic_weight: f64,
    /// What copper closer than the fab allows costs this variant.
    ///
    /// Zero everywhere but one variant. Step 4 of `docs/router-plan.md`
    /// measured it as the largest single improvement this vector has produced
    /// on one board and a regression of the same size on another, which is a
    /// variant's question rather than a default's.
    pub clearance_barrier: f64,
    /// What a via pays for each cell of another net's trace its copper
    /// touches. Zero everywhere but the variants that exist for it.
    pub via_touching_trace_penalty: f64,
    /// What a via pays per hole closer to it than the hole-to-hole rule
    /// allows. Zero everywhere but the variants that exist for it.
    pub via_near_hole_penalty: f64,
}

impl VariantConfig {
    /// A variant that differs from the defaults only in strategy and params.
    pub fn tuned(name: &str, strategy: StrategyKind, params: AutorouteParams) -> Self {
        Self {
            name: name.to_string(),
            strategy,
            params,
            via_ring_penalty: 0.0,
            pad_zone_blocks_foreign_copper: false,
            // Follows the default: reserving a trace's copper is what the
            // router does now, and a variant that differs in one knob should
            // differ in that knob alone.
            reserve_trace_footprint: true,
            foreign_pad_penalty: 0.0,
            pad_zone_margin_cells: cypcb_autoroute_default_margin(),
            heuristic_weight: 1.0,
            clearance_barrier: 0.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        }
    }
}

/// Result of a single routing variant, including score and serialized route data.
#[derive(Debug, Clone, Serialize)]
pub struct VariantResult {
    /// Name of the variant config that produced this result.
    pub name: String,
    /// Quality score for this variant.
    pub score: RoutingScore,
    /// Route segments produced by this variant.
    pub routes: Vec<RouteSegment>,
    /// Via placements produced by this variant.
    pub vias: Vec<ViaPlacement>,
    /// Connections this variant gave up on.
    ///
    /// Ranking cannot be left to the score alone: an abandoned connection
    /// removes copper, and copper is what earns violations and length, so a
    /// variant that quits on three nets outscores one that routes them. It was
    /// not hypothetical - `PathFinder Reserved Copper` won stm32_breakout
    /// while leaving 3 connections unrouted, and the score said 162,588
    /// against 287,564 as though that were the better board.
    pub unrouted: usize,
    /// How long this variant took, in milliseconds.
    ///
    /// Best-of-eight on `multi_ic` takes 86s in a release build against 5.9s
    /// for one run, and until this was carried out of the loop nothing said
    /// which of the eight the wait belonged to. Zero on wasm32, which has no
    /// clock this code may read.
    pub elapsed_ms: u64,
    /// Whether this is the winner with the repair pass run over it.
    ///
    /// The name stays the config's own, so everything that asks which
    /// settings won still gets an answer it can look up.
    pub repaired: bool,
}

/// Return the default set of variant configurations.
///
/// Returns 4 configs exercising different strategies and parameter combos:
/// 1. PathFinder default
/// 2. PathFinder low-via (via_cost=5.0)
/// 3. ImprovedAStar default
/// 4. PathFinder high-density (density=1.5)
pub fn default_variant_configs() -> Vec<VariantConfig> {
    let mut configs = vec![
        VariantConfig::tuned(
            "PathFinder Default",
            StrategyKind::PathFinder,
            AutorouteParams::default(),
        ),
        VariantConfig::tuned(
            "PathFinder Low-Via",
            StrategyKind::PathFinder,
            AutorouteParams {
                via_cost: 5.0,
                ..AutorouteParams::default()
            },
        ),
        VariantConfig::tuned(
            "PathFinder High-Density",
            StrategyKind::PathFinder,
            AutorouteParams {
                density: 1.5,
                ..AutorouteParams::default()
            },
        ),
        // A via kept off another net's trace. The keepout price charges a cell
        // whose copper would touch the via the same as one in the clearance
        // ring round it, and every trace-on-via short the winners carried on
        // stm32_breakout and qfp_fanout came from a layer change onto such
        // copper. Priced at 1000 per touching cell, measured on all six
        // fixtures against the thirteen variants before it, as shorts:
        //
        //   stm32_breakout  High-Density 28  ->  High-Density kept off 25
        //   qfp_fanout      High-Density 91  ->  Default kept off      34
        //
        // The other four boards keep their winners. Each of the two wins one
        // board and loses the other, so both are variants; a price of 2 was
        // measured too and won nowhere. `docs/routing.md` has the sweep.
        VariantConfig {
            via_touching_trace_penalty: 1000.0,
            ..VariantConfig::tuned(
                "PathFinder Vias Kept Off Traces",
                StrategyKind::PathFinder,
                AutorouteParams::default(),
            )
        },
        VariantConfig {
            via_touching_trace_penalty: 1000.0,
            ..VariantConfig::tuned(
                "PathFinder High-Density Vias Kept Off Traces",
                StrategyKind::PathFinder,
                AutorouteParams {
                    density: 1.5,
                    ..AutorouteParams::default()
                },
            )
        },
        // The two settings this project measured into existence. Neither is a
        // good default - each helps one benchmark board and hurts the other -
        // and that is exactly what a variant is for: the board picks, not the
        // author. multi_ic improves 18% under a priced via ring, and again
        // under a closed pad gate with a cheaper via; stm32_breakout rejects
        // both and keeps the first variant in this list.
        VariantConfig {
            name: "PathFinder Priced Via Rings".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams::default(),
            via_ring_penalty: 3.0,
            pad_zone_blocks_foreign_copper: false,
            reserve_trace_footprint: true,
            foreign_pad_penalty: 0.0,
            pad_zone_margin_cells: cypcb_autoroute_default_margin(),
            heuristic_weight: 1.0,
            clearance_barrier: 0.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        },
        VariantConfig {
            name: "PathFinder Guarded Pads".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams {
                via_cost: 0.5,
                ..AutorouteParams::default()
            },
            via_ring_penalty: 0.0,
            pad_zone_blocks_foreign_copper: true,
            reserve_trace_footprint: true,
            foreign_pad_penalty: 0.0,
            pad_zone_margin_cells: cypcb_autoroute_default_margin(),
            heuristic_weight: 1.0,
            clearance_barrier: 0.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        },
        // Reserving a trace's copper is the default since it was measured
        // better on every fixture and both columns. This is the control: the
        // router as it was, marking only the centre line the search walked.
        // Kept because a board that does worse under the reservation should
        // still have somewhere to go.
        // Crossing a neighbouring pin's copper priced rather than taken for
        // free. Measured: multi_ic 336 -> 267 violations and 166 -> 106
        // shorts, stm32_breakout 239 -> 280 - one board's gain and another's
        // loss, which is what a variant is for.
        //
        // **The price is 5, not the 20 it shipped at, and the reason is time.**
        // This variant wins `multi_ic` and was the slowest of the eight by
        // 6.6x - 32.2s of a 92s best-of-eight run. Swept on all six fixtures
        // (`pad_price_sweep`, release, 2026-08-08), price 5 against price 20,
        // as violations / shorts and seconds:
        //
        //   led_blink        1 / 1   0.0s      1 / 1    0.0s
        //   stm32_breakout 196 / 107 14.2s   248 / 147   8.9s
        //   multi_ic       180 / 101  8.6s   165 /  86  33.3s
        //   shift_driver    59 / 36   3.2s    59 /  36   3.1s
        //   plane_board     22 / 14   0.5s    22 /  14   0.5s
        //   qfp_fanout     448 / 265  4.6s   440 / 274   5.6s
        //
        // Read against each board's own noise band, 20 buys nothing: it is 15
        // violations better on `multi_ic` inside a band of 65, 52 *worse* on
        // `stm32_breakout` with 40 more shorts, identical on two boards and a
        // wash on `qfp_fanout`. What it costs is 24.7s on the one board where
        // this variant is chosen.
        VariantConfig {
            name: "PathFinder Pad Aware".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams::default(),
            via_ring_penalty: 0.0,
            pad_zone_blocks_foreign_copper: false,
            reserve_trace_footprint: true,
            foreign_pad_penalty: 5.0,
            pad_zone_margin_cells: cypcb_autoroute_default_margin(),
            heuristic_weight: 1.0,
            clearance_barrier: 0.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        },
        VariantConfig {
            name: "PathFinder Bare Centre Line".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams::default(),
            via_ring_penalty: 0.0,
            pad_zone_blocks_foreign_copper: false,
            reserve_trace_footprint: false,
            foreign_pad_penalty: 0.0,
            pad_zone_margin_cells: cypcb_autoroute_default_margin(),
            heuristic_weight: 1.0,
            clearance_barrier: 0.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        },
        // The opening around a pad, one cell narrower than the default. Every
        // cell of margin switches off obstacles that far from the pad, and on a
        // dense board that is the pin next door: stm32_breakout 239 -> 216
        // violations with 136 -> 86 shorts, multi_ic 336 -> 290 and 166 -> 131.
        // led_blink trades two near misses for one short, which is why this is
        // a variant rather than the default.
        // A search that stops insisting on the cheapest path.
        //
        // The estimate of the remaining distance is multiplied by 1.25, so the
        // search believes the goal is further than it is and follows the most
        // promising direction harder. It is not only faster - it is *better*
        // on the crowded boards, because the cheapest path by the cost
        // function is not the path with the fewest shorts. Measured against
        // the shipped weight of 1.0, violations and shorts:
        //
        //   led_blink        2 / 0  ->    3 / 1
        //   stm32_breakout 180 / 93 ->  163 / 70
        //   multi_ic       291 / 187 -> 227 / 115
        //   shift_driver    65 / 34 ->   69 / 29
        //   plane_board     28 / 13 ->   28 / 11
        //   qfp_fanout     309 / 147 -> 289 / 151
        //
        // Three boards clearly better, one clearly worse, which is what a
        // variant is for: `led_blink` keeps the optimal search because the
        // ranking puts shorts before everything but abandoned connections.
        VariantConfig {
            name: "PathFinder Eager".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams::default(),
            via_ring_penalty: 0.0,
            pad_zone_blocks_foreign_copper: false,
            reserve_trace_footprint: true,
            foreign_pad_penalty: 0.0,
            pad_zone_margin_cells: cypcb_autoroute_default_margin(),
            heuristic_weight: 1.25,
            clearance_barrier: 0.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        },
        // The two knobs that pay, together.
        //
        // `weight_and_pad_price_sweep` crosses the heuristic weight with the
        // foreign-pad price on all six fixtures, twelve points a board, and
        // these two combinations each take a board no single-knob variant can
        // reach. Violations and shorts, against the best any shipped variant
        // had managed before:
        //
        //   Eager Pads   multi_ic     167 / 83  against Pad Aware's 180 / 101
        //                plane_board   12 / 5   against 28 / 13
        //   Eager Light  shift_driver  62 / 20  against 65 / 34
        //
        // Both are bad elsewhere - `Eager Pads` gives qfp_fanout 426 / 261
        // against 309 / 147 - which is the whole reason they are variants and
        // not defaults. The board picks.
        VariantConfig {
            name: "PathFinder Eager Pads".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams::default(),
            via_ring_penalty: 0.0,
            pad_zone_blocks_foreign_copper: false,
            reserve_trace_footprint: true,
            foreign_pad_penalty: 20.0,
            pad_zone_margin_cells: cypcb_autoroute_default_margin(),
            heuristic_weight: 1.25,
            clearance_barrier: 0.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        },
        // Found 2026-08-21 by `is_the_best_variant_a_local_optimum`, which
        // moves one knob at a time around the point each board picks. Five of
        // the six boards' winners turned out to be local optima. `plane_board`
        // was not: the same `Eager Pads` with the via ring priced at 1 gives
        // **10 violations and 4 shorts against 12 and 5**, and that board's
        // measured noise band is **zero on both**, so the move is a setting
        // rather than the negotiation going differently.
        //
        // A thirteenth variant rather than a change to `Eager Pads`, because
        // `multi_ic` picks `Eager Pads` too and pricing its ring is not free
        // there. The board picks.
        VariantConfig {
            name: "PathFinder Eager Pads Priced Ring".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams::default(),
            via_ring_penalty: 1.0,
            pad_zone_blocks_foreign_copper: false,
            reserve_trace_footprint: true,
            foreign_pad_penalty: 20.0,
            pad_zone_margin_cells: cypcb_autoroute_default_margin(),
            heuristic_weight: 1.25,
            clearance_barrier: 0.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        },
        VariantConfig {
            name: "PathFinder Eager Light".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams::default(),
            via_ring_penalty: 0.0,
            pad_zone_blocks_foreign_copper: false,
            reserve_trace_footprint: true,
            foreign_pad_penalty: 5.0,
            pad_zone_margin_cells: cypcb_autoroute_default_margin(),
            heuristic_weight: 1.1,
            clearance_barrier: 0.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        },
        VariantConfig {
            name: "PathFinder Tight Pads".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams::default(),
            via_ring_penalty: 0.0,
            pad_zone_blocks_foreign_copper: false,
            reserve_trace_footprint: true,
            foreign_pad_penalty: 0.0,
            pad_zone_margin_cells: 2,
            heuristic_weight: 1.0,
            clearance_barrier: 0.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        },
        // The clearance barrier, priced. Step 4 of `docs/router-plan.md`
        // measured k = 10 as the largest single improvement this vector has
        // produced - `multi_ic` 262 / 194 to 123 / 85, against a band of
        // 65 / 56 - and a regression of the same size on `qfp_fanout`,
        // 318 / 149 to 424 / 269 against 57 / 44. Two boards, opposite
        // answers, which is a variant's question and not a default's: the
        // ranking picks per board and this gives it the point to pick.
        //
        // It is the only variant that costs the clearance field, which is
        // built once per board and is why `multi_ic` takes 6.9s here against
        // 0.5s elsewhere. Ranked complete-first like every other, so a slow
        // variant that abandons nothing still has to earn its place on shorts.
        VariantConfig {
            name: "PathFinder Clearance Priced".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams::default(),
            via_ring_penalty: 0.0,
            pad_zone_blocks_foreign_copper: false,
            reserve_trace_footprint: true,
            foreign_pad_penalty: 0.0,
            pad_zone_margin_cells: cypcb_autoroute_default_margin(),
            heuristic_weight: 1.0,
            clearance_barrier: 10.0,
            via_touching_trace_penalty: 0.0,
            via_near_hole_penalty: 0.0,
        },
    ];

    // A via kept its distance from every hole: another route's via, a pin, a
    // slot, a via the designer placed. Priced per hole closer than the
    // hole-to-hole rule allows, and measured on all six fixtures with the
    // price on every variant at once, 0 / 2 / 5 / 10 / 20 / 50 / 500. No price
    // won every board and the boards did not agree on one, so each variant
    // below carries the price its board ranked first, on the base it won with.
    // Winner before and after, as the ranking reads it - shorts, then
    // composite:
    //
    //   multi_ic      Clearance Priced   5    40 / 529284.4  ->  34 / 491270.5
    //   shift_driver  Priced Via Rings   2     0 /  14118.2  ->   0 /   9122.2
    //   stm32         High-Density       5    25 / 147172.7  ->  13 / 108670.2
    //
    // qfp_fanout has none: at every price its best board kept 35 shorts or
    // more against 34 without one. Never the default: price 5 on the fast path
    // took multi_ic from 472 violations to 523. The sweep is in
    // `docs/routing.md`.
    for (base, price) in [
        ("PathFinder Clearance Priced", 5.0),
        ("PathFinder Priced Via Rings", 2.0),
        ("PathFinder High-Density", 5.0),
    ] {
        let mut variant = configs
            .iter()
            .find(|config| config.name == base)
            .expect("a near-hole variant is built on a variant in this list")
            .clone();
        variant.name = format!("{base} Near Holes");
        variant.via_near_hole_penalty = price;
        configs.push(variant);
    }
    configs
}

/// The default opening, so a variant that does not care about it says so once.
fn cypcb_autoroute_default_margin() -> u16 {
    crate::orchestrator::DEFAULT_PAD_ZONE_MARGIN_CELLS
}

/// Full variant configs including ImprovedAStar — for native benchmarks only.
/// ImprovedAStar is too slow for WASM (blocks main thread for 20s+ on simple boards).
pub fn all_variant_configs() -> Vec<VariantConfig> {
    let mut configs = default_variant_configs();
    configs.push(VariantConfig::tuned(
        "ImprovedAStar Default",
        StrategyKind::ImprovedAStar,
        AutorouteParams::default(),
    ));
    configs
}

impl VariantResult {
    /// How far this variant is from a board whose every net is one piece of
    /// copper: the connections the router gave up on, plus the pins DRC finds
    /// no copper on and the nets DRC finds cut in two.
    ///
    /// The router's count alone missed what it laid badly. On `multi_ic` it
    /// ranked first a variant reporting 0 unrouted that DRC finds with 12
    /// bare pins and 3 split nets, over one with 4 and 0.
    pub fn incomplete(&self) -> usize {
        self.rank_key().incomplete
    }

    /// Where this variant stands in the ranking.
    pub fn rank_key(&self) -> RankKey {
        RankKey::of(self.unrouted, &self.score)
    }
}

/// What the ranking reads off a routed board, and the one order it reads it in.
///
/// The variants are ranked by it and the repair pass accepts an attempt by it,
/// so an attempt the pass keeps is one the ranking would also have put ahead.
/// The pass used to judge by its own count of contacts, and kept attempts the
/// ranking then placed below the board they started from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankKey {
    /// Connections given up on, bare pins and nets DRC finds cut in two.
    pub incomplete: usize,
    /// Copper touching copper.
    pub shorts: u32,
    /// Everything else, weighted.
    pub composite: f64,
}

impl RankKey {
    /// The key of a board the router left `unrouted` connections on.
    pub fn of(unrouted: usize, score: &RoutingScore) -> Self {
        RankKey {
            incomplete: unrouted + score.unrouted_pins as usize + score.net_splits as usize,
            shorts: score.shorts,
            composite: score.composite,
        }
    }

    /// Complete first, then fewest shorts, then the composite; `Less` is better.
    pub fn cmp_rank(&self, other: &Self) -> std::cmp::Ordering {
        // A complete board outranks an incomplete one whatever it scores, and
        // among incomplete ones fewer missing connections wins. Only then does
        // the composite decide. The alternative is a ranking that rewards giving
        // up, which is the same defect the CI regression gate was fixed for.
        self.incomplete
            .cmp(&other.incomplete)
            // Copper touching copper next, whatever the totals say. A board
            // with one short and one tight gap is not better than a board with
            // three tight gaps: the first cannot work and the second is a
            // yield risk. The composite charges every violation the same, so
            // the ordering has to make the distinction the score cannot.
            .then_with(|| self.shorts.cmp(&other.shorts))
            .then_with(|| {
                self.composite
                    .partial_cmp(&other.composite)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

/// Complete first, then fewest shorts, then the composite.
fn rank_best_first(results: &mut [VariantResult]) {
    results.sort_by(|a, b| a.rank_key().cmp_rank(&b.rank_key()));
}

/// Repair passes the winner gets once the variants are ranked.
const WINNER_REPAIR_PASSES: u32 = 2;

impl VariantConfig {
    /// How this variant routes, with `repair_passes` of repair after it.
    pub fn autoroute_config(&self, repair_passes: u32) -> AutorouteConfig {
        AutorouteConfig {
            strategy: self.strategy,
            params: self.params.clone(),
            via_ring_penalty: self.via_ring_penalty,
            pad_zone_blocks_foreign_copper: self.pad_zone_blocks_foreign_copper,
            reserve_trace_footprint: self.reserve_trace_footprint,
            foreign_pad_penalty: self.foreign_pad_penalty,
            pad_zone_margin_cells: self.pad_zone_margin_cells,
            heuristic_weight: self.heuristic_weight,
            clearance_barrier: self.clearance_barrier,
            via_touching_trace_penalty: self.via_touching_trace_penalty,
            via_near_hole_penalty: self.via_near_hole_penalty,
            repair_passes,
            ..AutorouteConfig::default()
        }
    }
}

/// The winner repaired, when the repair ranks ahead of it; otherwise `None`.
///
/// Measured on the seven benchmark boards on 2026-09-26: repair on every
/// variant cost 2.1x to 5.6x the ranking's wall clock on the boards that take
/// longer than a second, and on the winner alone at most 1.46x. The repaired
/// board is kept only when it ranks strictly ahead, so a board repair cannot
/// improve comes out exactly as it went in.
fn repair_winner(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    rules: &dyn RoutingRuleSet,
    design_rules: &DesignRules,
    config: &VariantConfig,
    winner: &VariantResult,
) -> Option<VariantResult> {
    // Repair re-routes a complete board; one with connections missing is
    // returned untouched, so there is nothing to spend.
    if winner.unrouted > 0 {
        return None;
    }
    #[cfg(not(target_arch = "wasm32"))]
    let start = std::time::Instant::now();

    clear_autorouted_traces(world);
    let initial = RoutingResult::complete(winner.routes.clone(), winner.vias.clone());
    let repaired = crate::repair::repair_routes(
        world,
        library,
        rules,
        &config.autoroute_config(WINNER_REPAIR_PASSES),
        initial,
    );

    apply_routes(world, &repaired);
    rebuild_spatial_index(world, library);
    let score = score_board(world, design_rules, &ScoreWeights::default());
    let candidate = VariantResult {
        name: winner.name.clone(),
        score,
        routes: repaired.routes,
        vias: repaired.vias,
        unrouted: 0,
        #[cfg(not(target_arch = "wasm32"))]
        elapsed_ms: winner.elapsed_ms + start.elapsed().as_millis() as u64,
        #[cfg(target_arch = "wasm32")]
        elapsed_ms: 0,
        repaired: true,
    };
    tracing::info!(
        name = %winner.name,
        before = ?winner.rank_key(),
        after = ?candidate.rank_key(),
        "Winner repaired"
    );
    (candidate.rank_key().cmp_rank(&winner.rank_key()) == std::cmp::Ordering::Less)
        .then_some(candidate)
}

/// Generate multiple routing variants sequentially on a single `&mut BoardWorld`.
///
/// For each config:
/// 1. Clear autorouted traces
/// 2. Route the board
/// 3. Apply routes to ECS (needed for scoring)
/// 4. Rebuild spatial index (needed for crossing detection)
/// 5. Score the board
/// 6. Capture routes/vias from the RoutingResult
/// 7. Store VariantResult
///
/// After all variants, sorts by composite score (ascending = best first)
/// and auto-applies the best variant to the world.
///
/// Individual variant failures are logged and skipped (not fatal).
pub fn generate_variants(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    rules: &dyn RoutingRuleSet,
    design_rules: &DesignRules,
    configs: &[VariantConfig],
) -> Vec<VariantResult> {
    let _span = tracing::info_span!("generate_variants", count = configs.len()).entered();

    #[cfg(not(target_arch = "wasm32"))]
    let start = std::time::Instant::now();

    let weights = ScoreWeights::default();
    let mut results: Vec<VariantResult> = Vec::with_capacity(configs.len());

    for config in configs {
        #[cfg(not(target_arch = "wasm32"))]
        let variant_start = std::time::Instant::now();

        let variant_span =
            tracing::info_span!("variant", name = %config.name, strategy = %config.strategy)
                .entered();

        // 1. Clear previous variant's entities
        clear_autorouted_traces(world);

        // 2. Route the board with this config. Exploration compares many
        // routings, and repair on each one costs 2.1x to 5.6x the wall clock to
        // rank candidates that are about to be thrown away, so only the winner
        // is repaired, after the ranking - see `repair_winner`.
        let autoroute_config = config.autoroute_config(0);

        let routing_result = route_board(world, library, rules, &autoroute_config);

        // Check if routing failed entirely
        if routing_result.status.is_failed() {
            tracing::warn!(
                name = %config.name,
                "Variant routing failed, skipping"
            );
            drop(variant_span);
            continue;
        }

        // Capture routes/vias from the RoutingResult BEFORE applying
        // (apply_routes groups segments by net+layer, so we want the raw data)
        let routes = routing_result.routes.clone();
        let vias = routing_result.vias.clone();

        // 3. Apply routes to ECS so score_board() can query entities
        apply_routes(world, &routing_result);

        // 4. Rebuild spatial index with traces for crossing detection
        rebuild_spatial_index(world, library);

        // 5. Score the board
        let score = score_board(world, design_rules, &weights);

        tracing::info!(
            name = %config.name,
            composite = score.composite,
            route_count = routes.len(),
            via_count = vias.len(),
            "Variant scored"
        );

        results.push(VariantResult {
            name: config.name.clone(),
            score,
            routes,
            vias,
            unrouted: match routing_result.status {
                RoutingStatus::Partial { unrouted_count } => unrouted_count,
                _ => 0,
            },
            #[cfg(not(target_arch = "wasm32"))]
            elapsed_ms: variant_start.elapsed().as_millis() as u64,
            #[cfg(target_arch = "wasm32")]
            elapsed_ms: 0,
            repaired: false,
        });

        drop(variant_span);
    }

    rank_best_first(&mut results);

    let winner_config = results
        .first()
        .and_then(|best| configs.iter().find(|config| config.name == best.name));
    if let Some(config) = winner_config {
        if let Some(repaired) =
            repair_winner(world, library, rules, design_rules, config, &results[0])
        {
            results.insert(0, repaired);
        }
    }

    // Apply the best variant to the world
    if let Some(best) = results.first() {
        #[cfg(not(target_arch = "wasm32"))]
        tracing::info!(
            best_name = %best.name,
            best_composite = best.score.composite,
            variant_count = results.len(),
            elapsed_ms = start.elapsed().as_millis() as u64,
            "Variant generation complete, applying best"
        );
        #[cfg(target_arch = "wasm32")]
        tracing::info!(
            best_name = %best.name,
            best_composite = best.score.composite,
            variant_count = results.len(),
            "Variant generation complete, applying best"
        );

        // Clear and re-apply the best variant
        clear_autorouted_traces(world);
        let best_result = RoutingResult::complete(best.routes.clone(), best.vias.clone());
        apply_routes(world, &best_result);
        rebuild_spatial_index(world, library);
    } else {
        tracing::warn!("No variants succeeded, world left with no routes");
    }

    results
}

/// Clear autorouted traces and vias from the world.
fn clear_autorouted_traces(world: &mut BoardWorld) {
    use cypcb_world::components::trace::{RouterPlaced, Trace, TraceSource, Via};
    use cypcb_world::Entity;

    let entities_to_remove: Vec<Entity> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(Entity, &Trace)>();
        query
            .iter(ecs)
            .filter(|(_, trace)| trace.source == TraceSource::Autorouted && !trace.locked)
            .map(|(entity, _)| entity)
            .collect()
    };

    // Vias this router put down, and only those - the same question
    // `apply_routes` asks. This used to ask `!via.locked`, so every variant was
    // routed and scored on a board without the vias the file declares: on
    // `blind-via` each composite came out 2002 lower than the same routes
    // scored on the board as written, on `stitched-plane` 16 lower.
    let via_entities: Vec<Entity> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(Entity, &Via, &RouterPlaced)>();
        query
            .iter(ecs)
            .filter(|(_, via, _)| !via.locked)
            .map(|(entity, _, _)| entity)
            .collect()
    };

    let ecs = world.ecs_mut();
    for entity in entities_to_remove {
        ecs.despawn(entity);
    }
    for entity in via_entities {
        ecs.despawn(entity);
    }
}

/// Rebuild spatial index including traces.
fn rebuild_spatial_index(world: &mut BoardWorld, library: &FootprintLibrary) {
    world.rebuild_spatial_index_from_library(library);
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Nm;

    #[test]
    fn default_variant_configs_cover_the_measured_settings() {
        // The count is not the point - the coverage is. Every setting this
        // project measured into a per-board win has to be reachable, or a
        // board that needs it never gets it.
        let configs = default_variant_configs();
        let names: Vec<&str> = configs.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names[0], "PathFinder Default", "the baseline comes first");
        assert!(names.contains(&"PathFinder Low-Via"));
        assert!(names.contains(&"PathFinder High-Density"));
        assert!(
            configs.iter().any(|c| c.via_ring_penalty > 0.0),
            "a variant has to price via rings: multi_ic improves 18% under it"
        );
        assert!(
            configs.iter().any(|c| c.pad_zone_blocks_foreign_copper),
            "a variant has to guard pad keepouts: it is multi_ic's best result"
        );
        assert!(
            configs.iter().any(|c| c.foreign_pad_penalty > 0.0),
            "a variant has to price a foreign pad: multi_ic picks it at 267/106 against 336/166"
        );
        assert!(
            configs
                .iter()
                .any(|c| c.pad_zone_margin_cells
                    < crate::orchestrator::DEFAULT_PAD_ZONE_MARGIN_CELLS),
            "a variant has to narrow the pad opening: stm32_breakout picks it at 216/86"
        );
        assert!(
            configs
                .iter()
                .any(|c| c.via_near_hole_penalty > 0.0 && c.name.ends_with("Near Holes")),
            "no variant keeps a via off the holes"
        );
    }

    #[test]
    fn the_near_hole_price_is_a_variant_and_never_the_default() {
        assert_eq!(AutorouteConfig::default().via_near_hole_penalty, 0.0);
        assert_eq!(
            VariantConfig::tuned("x", StrategyKind::PathFinder, AutorouteParams::default())
                .via_near_hole_penalty,
            0.0
        );
    }

    #[test]
    fn all_variant_configs_includes_improved_astar() {
        let configs = all_variant_configs();
        assert_eq!(configs.len(), default_variant_configs().len() + 1);
        assert!(configs
            .iter()
            .any(|c| c.strategy == StrategyKind::ImprovedAStar));
    }

    #[test]
    fn default_configs_have_expected_strategies() {
        let configs = default_variant_configs();
        assert_eq!(configs[0].strategy, StrategyKind::PathFinder);
        assert_eq!(configs[1].strategy, StrategyKind::PathFinder);
        assert_eq!(configs[2].strategy, StrategyKind::PathFinder);
    }

    #[test]
    fn default_configs_have_expected_params() {
        let configs = default_variant_configs();
        // PathFinder Low-Via has via_cost=5.0
        assert_eq!(configs[1].params.via_cost, 5.0);
        // PathFinder High-Density has density=1.5
        assert_eq!(configs[2].params.density, 1.5);
    }

    #[test]
    fn variant_result_serializes_to_json() {
        let result = VariantResult {
            name: "Test Variant".to_string(),
            score: RoutingScore {
                total_length: Nm::from_mm(100.0),
                via_count: 3,
                drc_violations: 0,
                shorts: 0,
                smoothness: 0.95,
                crossings: 1,
                layer_balance: 0.8,
                composite: 42.5,
                clearance_contacts: 0,
                unrouted_pins: 0,
                net_splits: 0,
            },
            routes: vec![RouteSegment::new(
                cypcb_world::NetId::new(1),
                cypcb_world::Layer::TopCopper,
                Nm::from_mm(0.2),
                cypcb_core::Point::from_mm(0.0, 0.0),
                cypcb_core::Point::from_mm(10.0, 0.0),
            )],
            vias: vec![ViaPlacement::through_hole(
                cypcb_world::NetId::new(1),
                cypcb_core::Point::from_mm(5.0, 5.0),
                Nm::from_mm(0.3),
            )],
            unrouted: 0,
            elapsed_ms: 0,
            repaired: false,
        };

        let json = serde_json::to_string(&result).expect("VariantResult should serialize");
        assert!(json.contains("\"name\":\"Test Variant\""));
        assert!(json.contains("\"composite\":42.5"));
        assert!(json.contains("\"routes\":["));
        assert!(json.contains("\"vias\":["));
    }

    #[test]
    fn variant_result_vec_serializes() {
        let results = vec![
            VariantResult {
                name: "A".to_string(),
                score: RoutingScore {
                    total_length: Nm(0),
                    via_count: 0,
                    drc_violations: 0,
                    shorts: 0,
                    smoothness: 1.0,
                    crossings: 0,
                    layer_balance: 1.0,
                    composite: 10.0,
                    clearance_contacts: 0,
                    unrouted_pins: 0,
                    net_splits: 0,
                },
                routes: vec![],
                vias: vec![],
                unrouted: 0,
                elapsed_ms: 0,
                repaired: false,
            },
            VariantResult {
                name: "B".to_string(),
                score: RoutingScore {
                    total_length: Nm(0),
                    via_count: 0,
                    drc_violations: 0,
                    shorts: 0,
                    smoothness: 1.0,
                    crossings: 0,
                    layer_balance: 1.0,
                    composite: 20.0,
                    clearance_contacts: 0,
                    unrouted_pins: 0,
                    net_splits: 0,
                },
                routes: vec![],
                vias: vec![],
                unrouted: 0,
                elapsed_ms: 0,
                repaired: false,
            },
        ];

        let json = serde_json::to_string(&results).expect("Vec<VariantResult> should serialize");
        // Should be a JSON array
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
        assert!(json.contains("\"name\":\"A\""));
        assert!(json.contains("\"name\":\"B\""));
    }

    fn ranked(name: &str, unrouted: usize, net_splits: u32, composite: f64) -> VariantResult {
        VariantResult {
            name: name.to_string(),
            score: RoutingScore {
                total_length: Nm(0),
                via_count: 0,
                drc_violations: net_splits,
                shorts: 0,
                smoothness: 1.0,
                crossings: 0,
                layer_balance: 1.0,
                composite,
                clearance_contacts: 0,
                unrouted_pins: 0,
                net_splits,
            },
            routes: vec![],
            vias: vec![],
            unrouted,
            elapsed_ms: 0,
            repaired: false,
        }
    }

    #[test]
    fn a_net_cut_in_two_is_not_complete_whatever_the_router_says() {
        // The router reports nothing unrouted for both. DRC finds the first
        // one's net in two pieces, and it scores better, because the copper
        // it did not lay costs no length and no violations.
        let mut results = vec![ranked("split", 0, 1, 10.0), ranked("whole", 0, 0, 20.0)];
        rank_best_first(&mut results);
        assert_eq!(
            results[0].name, "whole",
            "a board with a net in two pieces won"
        );
    }
}
