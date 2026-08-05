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

use cypcb_core::Nm;
use cypcb_drc::DesignRules;
use cypcb_router::apply_routes;
use cypcb_router::types::{RouteSegment, RoutingResult, ViaPlacement};
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
}

/// Return the default set of variant configurations.
///
/// Returns 4 configs exercising different strategies and parameter combos:
/// 1. PathFinder default
/// 2. PathFinder low-via (via_cost=5.0)
/// 3. ImprovedAStar default
/// 4. PathFinder high-density (density=1.5)
pub fn default_variant_configs() -> Vec<VariantConfig> {
    vec![
        VariantConfig {
            name: "PathFinder Default".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams::default(),
        },
        VariantConfig {
            name: "PathFinder Low-Via".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams {
                via_cost: 5.0,
                ..AutorouteParams::default()
            },
        },
        VariantConfig {
            name: "PathFinder High-Density".to_string(),
            strategy: StrategyKind::PathFinder,
            params: AutorouteParams {
                density: 1.5,
                ..AutorouteParams::default()
            },
        },
    ]
}

/// Full variant configs including ImprovedAStar — for native benchmarks only.
/// ImprovedAStar is too slow for WASM (blocks main thread for 20s+ on simple boards).
pub fn all_variant_configs() -> Vec<VariantConfig> {
    let mut configs = default_variant_configs();
    configs.push(VariantConfig {
        name: "ImprovedAStar Default".to_string(),
        strategy: StrategyKind::ImprovedAStar,
        params: AutorouteParams::default(),
    });
    configs
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
        let variant_span =
            tracing::info_span!("variant", name = %config.name, strategy = %config.strategy)
                .entered();

        // 1. Clear previous variant's entities
        clear_autorouted_traces(world);

        // 2. Route the board with this config
        let autoroute_config = AutorouteConfig {
            strategy: config.strategy,
            params: config.params.clone(),
            ..AutorouteConfig::default()
        };

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
        });

        drop(variant_span);
    }

    // Sort by composite score ascending (best = lowest first)
    results.sort_by(|a, b| {
        a.score
            .composite
            .partial_cmp(&b.score.composite)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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
    use cypcb_world::components::trace::{Trace, TraceSource, Via};
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

    let via_entities: Vec<Entity> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(Entity, &Via)>();
        query
            .iter(ecs)
            .filter(|(_, via)| !via.locked)
            .map(|(entity, _)| entity)
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
    world.rebuild_spatial_index_with_traces(|name| {
        library.get(name).map(|fp| fp.courtyard).unwrap_or_else(|| {
            cypcb_core::Rect::from_center_size(
                cypcb_core::Point::ORIGIN,
                (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            )
        })
    });
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_variant_configs_returns_3() {
        let configs = default_variant_configs();
        assert_eq!(configs.len(), 3);
        assert_eq!(configs[0].name, "PathFinder Default");
        assert_eq!(configs[1].name, "PathFinder Low-Via");
        assert_eq!(configs[2].name, "PathFinder High-Density");
    }

    #[test]
    fn all_variant_configs_includes_improved_astar() {
        let configs = all_variant_configs();
        assert_eq!(configs.len(), 4);
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
                smoothness: 0.95,
                crossings: 1,
                layer_balance: 0.8,
                composite: 42.5,
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
                    smoothness: 1.0,
                    crossings: 0,
                    layer_balance: 1.0,
                    composite: 10.0,
                },
                routes: vec![],
                vias: vec![],
            },
            VariantResult {
                name: "B".to_string(),
                score: RoutingScore {
                    total_length: Nm(0),
                    via_count: 0,
                    drc_violations: 0,
                    smoothness: 1.0,
                    crossings: 0,
                    layer_balance: 1.0,
                    composite: 20.0,
                },
                routes: vec![],
                vias: vec![],
            },
        ];

        let json = serde_json::to_string(&results).expect("Vec<VariantResult> should serialize");
        // Should be a JSON array
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
        assert!(json.contains("\"name\":\"A\""));
        assert!(json.contains("\"name\":\"B\""));
    }
}
