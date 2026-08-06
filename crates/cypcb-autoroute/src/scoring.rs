//! Routing quality scoring module.
//!
//! Provides a composite quality score for routed PCB boards based on 7 metrics:
//! trace length, via count, DRC violations, smoothness, crossings, layer balance,
//! and a weighted composite score.
//!
//! # Usage
//!
//! ```rust,ignore
//! use cypcb_autoroute::scoring::{score_board, ScoreWeights};
//! use cypcb_drc::DesignRules;
//! use cypcb_world::BoardWorld;
//!
//! let mut world = BoardWorld::new();
//! // ... build and route the board ...
//!
//! let rules = DesignRules::jlcpcb_2layer();
//! let score = score_board(&mut world, &rules, &ScoreWeights::default());
//! println!("Composite: {}", score.composite);
//! ```
//!
//! # Metric Semantics
//!
//! - **total_length**: Sum of all trace segment lengths (Nm). Lower = better.
//! - **via_count**: Number of Via entities. Lower = better.
//! - **drc_violations**: Number of DRC rule violations. Lower = better (0 = target).
//! - **smoothness**: 0.0–1.0 where 1.0 = all bends at 45° multiples. Higher = better.
//! - **crossings**: Same-layer inter-net segment intersections. Lower = better (0 = target).
//! - **layer_balance**: min/max ratio of per-layer trace counts. 1.0 = balanced. Higher = better.
//! - **composite**: Weighted sum of all metrics, normalized. Lower = better.

use bevy_ecs::prelude::*;
use serde::Serialize;

use cypcb_core::Nm;
use cypcb_drc::rules::clearance::segment_distance;
use cypcb_drc::{run_drc, DesignRules};
use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::BoardWorld;

/// Quality score for a routed PCB board.
///
/// Contains 7 individually-computed metrics plus a weighted composite score.
/// Lower composite = better routing quality.
#[derive(Debug, Clone, Serialize)]
pub struct RoutingScore {
    /// Total routed trace length in nanometers.
    pub total_length: Nm,
    /// Number of vias on the board.
    pub via_count: u32,
    /// Number of DRC violations.
    pub drc_violations: u32,
    /// Smoothness score (0.0–1.0). 1.0 = all bends at 45° multiples.
    pub smoothness: f64,
    /// Number of same-layer inter-net segment crossings.
    pub crossings: u32,
    /// Layer balance ratio (0.0–1.0). 1.0 = traces evenly distributed across layers.
    pub layer_balance: f64,
    /// Weighted composite score. Lower = better.
    pub composite: f64,
}

/// Configurable weights for the composite score formula.
///
/// Each weight controls the relative importance of its metric in the composite.
/// All weights should be non-negative. The default uses equal weights of 1.0.
#[derive(Debug, Clone, Serialize)]
pub struct ScoreWeights {
    pub length: f64,
    pub via: f64,
    pub drc: f64,
    pub smoothness: f64,
    pub crossings: f64,
    pub balance: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        ScoreWeights {
            length: 1.0,
            via: 1.0,
            drc: 1.0,
            smoothness: 1.0,
            crossings: 1.0,
            balance: 1.0,
        }
    }
}

/// Configuration for the scoring system.
#[derive(Debug, Clone, Default)]
pub struct ScoringConfig {
    pub weights: ScoreWeights,
}

/// Score a routed board on all 7 metrics.
///
/// Queries ECS entities for traces and vias, runs DRC, computes smoothness,
/// crossing count, layer balance, and a weighted composite score.
///
/// # Arguments
///
/// * `world` - Board world with routed traces (mutable for ECS queries and DRC)
/// * `rules` - Design rules for DRC checking
/// * `weights` - Weights for the composite score formula
///
/// # Returns
///
/// A [`RoutingScore`] with all 7 metrics populated.
/// # Prerequisite
///
/// The caller must have built the spatial index over the routed board, with
/// [`BoardWorld::rebuild_spatial_index_from_library`]. Both the DRC count and
/// the crossing count read that index, and an index sized from anything other
/// than the real footprints under-reports both.
pub fn score_board(
    world: &mut BoardWorld,
    rules: &DesignRules,
    weights: &ScoreWeights,
) -> RoutingScore {
    let _span = tracing::debug_span!("score_board").entered();

    // Collect trace data from ECS
    let traces: Vec<TraceData> = {
        let mut query = world.ecs_mut().query::<(Entity, &Trace)>();
        query
            .iter(world.ecs())
            .map(|(entity, trace)| TraceData {
                entity,
                segments: trace.segments.clone(),
                layer: trace.layer,
                net_id: trace.net_id,
            })
            .collect()
    };

    // 1. Total trace length
    let total_length: i64 = traces.iter().map(|t| t.total_length()).sum();
    let total_length = Nm(total_length);

    // 2. Via count
    let via_count: u32 = {
        let mut query = world.ecs_mut().query::<&Via>();
        query.iter(world.ecs()).count() as u32
    };

    // 3. DRC violations
    let drc_result = run_drc(world, rules);
    let drc_violations = drc_result.violation_count() as u32;

    // 4. Smoothness
    let smoothness = compute_smoothness(&traces);

    // 5. Crossings (rebuild spatial index to include traces)
    let crossings = compute_crossings(world, &traces);

    // 6. Layer balance
    let layer_balance = compute_layer_balance(&traces);

    // 7. Composite
    let board_diagonal = board_diagonal_nm(world);
    let composite = compute_composite(
        total_length,
        via_count,
        drc_violations,
        smoothness,
        crossings,
        layer_balance,
        board_diagonal,
        weights,
    );

    tracing::debug!(
        total_length_mm = total_length.0 as f64 / 1_000_000.0,
        via_count,
        drc_violations,
        smoothness,
        crossings,
        layer_balance,
        composite,
        "Board scoring complete"
    );

    RoutingScore {
        total_length,
        via_count,
        drc_violations,
        smoothness,
        crossings,
        layer_balance,
        composite,
    }
}

// ============================================================================
// Internal data structures
// ============================================================================

use cypcb_world::components::trace::TraceSegment;
use cypcb_world::Layer;
use cypcb_world::NetId;

/// Collected trace data for metric computation.
struct TraceData {
    entity: Entity,
    segments: Vec<TraceSegment>,
    layer: Layer,
    net_id: NetId,
}

impl TraceData {
    fn total_length(&self) -> i64 {
        self.segments.iter().map(|s| s.length().0).sum()
    }
}

// ============================================================================
// Smoothness metric
// ============================================================================

/// Compute the angle penalty for a bend angle in radians.
///
/// Returns a value in [0.0, 1.0] measuring how far the angle deviates
/// from the nearest 45° multiple. 0.0 = on a 45° multiple (ideal),
/// 1.0 = maximally off (22.5° from nearest multiple).
pub(crate) fn angle_penalty(angle_rad: f64) -> f64 {
    let deg45 = std::f64::consts::FRAC_PI_4; // π/4 = 45°
                                             // Normalize angle to [0, π/4) range by taking modulo 45°
    let remainder = (angle_rad.abs() % deg45).min(deg45 - (angle_rad.abs() % deg45));
    // Max deviation from a 45° multiple is 22.5° = π/8
    let max_deviation = deg45 / 2.0;
    (remainder / max_deviation).min(1.0)
}

/// Compute smoothness across all traces.
///
/// For each trace, iterates consecutive segment pairs, computes the bend angle,
/// and accumulates angle penalties. Returns 1.0 - (total_penalty / total_bends).
///
/// Edge cases:
/// - Empty board (no traces) → 1.0 (perfect)
/// - Traces with 0 or 1 segment (no bends) → 1.0 (perfect)
/// - Zero-length segments (start == end) → skipped
fn compute_smoothness(traces: &[TraceData]) -> f64 {
    let mut total_penalty = 0.0;
    let mut total_bends = 0u32;

    for trace in traces {
        if trace.segments.len() < 2 {
            continue;
        }

        for window in trace.segments.windows(2) {
            let seg_a = &window[0];
            let seg_b = &window[1];

            // Skip zero-length segments
            let dx_a = seg_a.end.x.0 - seg_a.start.x.0;
            let dy_a = seg_a.end.y.0 - seg_a.start.y.0;
            if dx_a == 0 && dy_a == 0 {
                continue;
            }

            let dx_b = seg_b.end.x.0 - seg_b.start.x.0;
            let dy_b = seg_b.end.y.0 - seg_b.start.y.0;
            if dx_b == 0 && dy_b == 0 {
                continue;
            }

            // Compute bend angle between the two direction vectors
            let angle_a = (dy_a as f64).atan2(dx_a as f64);
            let angle_b = (dy_b as f64).atan2(dx_b as f64);
            let bend = angle_b - angle_a;

            // Normalize to [-π, π]
            let bend = bend.rem_euclid(2.0 * std::f64::consts::PI);
            let bend = if bend > std::f64::consts::PI {
                bend - 2.0 * std::f64::consts::PI
            } else {
                bend
            };

            total_penalty += angle_penalty(bend);
            total_bends += 1;
        }
    }

    if total_bends == 0 {
        1.0
    } else {
        1.0 - (total_penalty / total_bends as f64)
    }
}

// ============================================================================
// Crossing detection
// ============================================================================

/// Count same-layer inter-net segment crossings.
///
/// Uses the spatial index for efficient candidate selection. For each trace
/// segment, queries nearby segments on the same layer. Only counts crossings
/// between different nets (same-net junctions are intentional connections).
///
/// Uses canonical entity+segment pair ordering to avoid double-counting.
fn compute_crossings(world: &mut BoardWorld, traces: &[TraceData]) -> u32 {
    use cypcb_core::{Point, Rect};
    use std::collections::HashSet;

    // The caller owns the index. This used to rebuild it here, sizing every
    // component as a fixed 1mm box - which both discarded whatever the caller
    // had built and left the world holding an index that hides violations on
    // any component larger than a millimetre.

    // Build entity → (layer, net_id, segments) lookup
    let mut trace_lookup: std::collections::HashMap<Entity, &TraceData> =
        std::collections::HashMap::new();
    for td in traces {
        trace_lookup.insert(td.entity, td);
    }

    let mut crossings: u32 = 0;
    // Track checked pairs to avoid double-counting: (entity_a, seg_idx_a, entity_b, seg_idx_b)
    let mut checked: HashSet<(u32, usize, u32, usize)> = HashSet::new();

    for trace in traces {
        let layer_mask = trace.layer.to_copper_mask();

        for (seg_idx, seg) in trace.segments.iter().enumerate() {
            // Build query AABB around this segment
            let min_x = seg.start.x.0.min(seg.end.x.0);
            let min_y = seg.start.y.0.min(seg.end.y.0);
            let max_x = seg.start.x.0.max(seg.end.x.0);
            let max_y = seg.start.y.0.max(seg.end.y.0);

            let query_region = Rect::from_points(
                Point::new(Nm(min_x), Nm(min_y)),
                Point::new(Nm(max_x), Nm(max_y)),
            );

            // Query spatial index for nearby entities on same layer
            let candidates = world.query_region_on_layers(query_region, layer_mask);

            for candidate_entity in candidates {
                // Skip self
                if candidate_entity == trace.entity {
                    continue;
                }

                // Look up candidate trace data
                let candidate = match trace_lookup.get(&candidate_entity) {
                    Some(td) => td,
                    None => continue, // Not a trace entity (pad, component, etc.)
                };

                // Skip same-net (junctions, not crossings)
                if candidate.net_id == trace.net_id {
                    continue;
                }

                // Check each candidate segment for intersection
                for (cand_seg_idx, cand_seg) in candidate.segments.iter().enumerate() {
                    // Canonical pair ordering to avoid double-counting
                    let pair = if trace.entity.index() < candidate_entity.index()
                        || (trace.entity.index() == candidate_entity.index()
                            && seg_idx < cand_seg_idx)
                    {
                        (
                            trace.entity.index(),
                            seg_idx,
                            candidate_entity.index(),
                            cand_seg_idx,
                        )
                    } else {
                        (
                            candidate_entity.index(),
                            cand_seg_idx,
                            trace.entity.index(),
                            seg_idx,
                        )
                    };

                    if !checked.insert(pair) {
                        continue;
                    }

                    // Use segment_distance to detect intersection (distance == 0)
                    let dist = segment_distance(
                        [seg.start.x.0, seg.start.y.0],
                        [seg.end.x.0, seg.end.y.0],
                        [cand_seg.start.x.0, cand_seg.start.y.0],
                        [cand_seg.end.x.0, cand_seg.end.y.0],
                    );

                    if dist == 0 {
                        crossings += 1;
                    }
                }
            }
        }
    }

    crossings
}

// ============================================================================
// Layer balance
// ============================================================================

/// Compute layer balance as min(counts) / max(counts).
///
/// Returns 1.0 for single-layer boards or boards with no traces.
fn compute_layer_balance(traces: &[TraceData]) -> f64 {
    if traces.is_empty() {
        return 1.0;
    }

    let mut layer_counts: std::collections::HashMap<Layer, u32> = std::collections::HashMap::new();
    for trace in traces {
        *layer_counts.entry(trace.layer).or_insert(0) += 1;
    }

    if layer_counts.len() <= 1 {
        // Single layer — balanced by definition
        return 1.0;
    }

    let min_count = *layer_counts.values().min().unwrap_or(&0);
    let max_count = *layer_counts.values().max().unwrap_or(&1);

    if max_count == 0 {
        return 1.0;
    }

    min_count as f64 / max_count as f64
}

// ============================================================================
// Composite score
// ============================================================================

/// Get the board diagonal in nanometers for length normalization.
///
/// Returns a default of 100mm diagonal if no board is set.
fn board_diagonal_nm(world: &BoardWorld) -> f64 {
    if let Some((size, _)) = world.board_info() {
        let w = size.width.0 as f64;
        let h = size.height.0 as f64;
        (w * w + h * h).sqrt()
    } else {
        // Default: 100mm diagonal
        100_000_000.0
    }
}

/// Compute the weighted composite score. Lower = better.
///
/// Formula:
/// ```text
/// composite = w_length * (total_length / board_diagonal)
///           + w_via * via_count
///           + w_drc * drc_violations * 1000
///           + w_smoothness * (1.0 - smoothness) * 100
///           + w_crossings * crossings * 500
///           + w_balance * (1.0 - layer_balance) * 50
/// ```
#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_composite(
    total_length: Nm,
    via_count: u32,
    drc_violations: u32,
    smoothness: f64,
    crossings: u32,
    layer_balance: f64,
    board_diagonal: f64,
    weights: &ScoreWeights,
) -> f64 {
    let norm_length = if board_diagonal > 0.0 {
        total_length.0 as f64 / board_diagonal
    } else {
        0.0
    };

    weights.length * norm_length
        + weights.via * via_count as f64
        + weights.drc * drc_violations as f64 * 1000.0
        + weights.smoothness * (1.0 - smoothness) * 100.0
        + weights.crossings * crossings as f64 * 500.0
        + weights.balance * (1.0 - layer_balance) * 50.0
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::{Nm, Point};
    use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
    use cypcb_world::footprint::FootprintLibrary;
    use cypcb_world::Layer;
    use cypcb_world::NetId;

    // ====================================================================
    // Angle penalty tests
    // ====================================================================

    #[test]
    fn test_angle_penalty_0_degrees() {
        // 0° = on a 45° multiple (0° is a multiple of 45°)
        let penalty = angle_penalty(0.0);
        assert!(
            penalty.abs() < 1e-10,
            "0° should have zero penalty, got {penalty}"
        );
    }

    #[test]
    fn test_angle_penalty_45_degrees() {
        let penalty = angle_penalty(std::f64::consts::FRAC_PI_4);
        assert!(
            penalty.abs() < 1e-10,
            "45° should have zero penalty, got {penalty}"
        );
    }

    #[test]
    fn test_angle_penalty_90_degrees() {
        let penalty = angle_penalty(std::f64::consts::FRAC_PI_2);
        assert!(
            penalty.abs() < 1e-10,
            "90° should have zero penalty, got {penalty}"
        );
    }

    #[test]
    fn test_angle_penalty_23_degrees() {
        // 23° is between 0° and 45°, closest to 22.5° from both
        let angle = 23.0_f64.to_radians();
        let penalty = angle_penalty(angle);
        // 23° is 23° from 0° and 22° from 45°. Closest multiple is 0° at 23°.
        // Penalty = 23/22.5 ≈ 1.02 → clamped to 1.0? No:
        // remainder = min(23%45, 45-23%45) = min(23, 22) = 22
        // penalty = 22/22.5 ≈ 0.978
        assert!(
            penalty > 0.9 && penalty < 1.0,
            "23° should have high penalty (~0.978), got {penalty}"
        );
    }

    #[test]
    fn test_angle_penalty_22_5_degrees() {
        // 22.5° is maximally between 0° and 45° → penalty = 1.0
        let angle = 22.5_f64.to_radians();
        let penalty = angle_penalty(angle);
        assert!(
            (penalty - 1.0).abs() < 1e-10,
            "22.5° should have max penalty 1.0, got {penalty}"
        );
    }

    #[test]
    fn test_angle_penalty_negative() {
        // Negative angles should be treated same as positive
        let penalty_pos = angle_penalty(std::f64::consts::FRAC_PI_4);
        let penalty_neg = angle_penalty(-std::f64::consts::FRAC_PI_4);
        assert!(
            (penalty_pos - penalty_neg).abs() < 1e-10,
            "Negative angles should have same penalty as positive"
        );
    }

    // ====================================================================
    // Layer balance tests
    // ====================================================================

    fn make_trace_data(layer: Layer, net_id: u32) -> TraceData {
        TraceData {
            entity: Entity::from_raw(net_id),
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            )],
            layer,
            net_id: NetId::new(net_id),
        }
    }

    #[test]
    fn test_layer_balance_empty() {
        assert!((compute_layer_balance(&[]) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_layer_balance_single_layer() {
        let traces = vec![
            make_trace_data(Layer::TopCopper, 0),
            make_trace_data(Layer::TopCopper, 1),
            make_trace_data(Layer::TopCopper, 2),
        ];
        assert!(
            (compute_layer_balance(&traces) - 1.0).abs() < 1e-10,
            "Single layer should be 1.0"
        );
    }

    #[test]
    fn test_layer_balance_perfectly_balanced() {
        let traces = vec![
            make_trace_data(Layer::TopCopper, 0),
            make_trace_data(Layer::TopCopper, 1),
            make_trace_data(Layer::BottomCopper, 2),
            make_trace_data(Layer::BottomCopper, 3),
        ];
        assert!(
            (compute_layer_balance(&traces) - 1.0).abs() < 1e-10,
            "2+2 should be perfectly balanced"
        );
    }

    #[test]
    fn test_layer_balance_imbalanced() {
        let traces = vec![
            make_trace_data(Layer::TopCopper, 0),
            make_trace_data(Layer::TopCopper, 1),
            make_trace_data(Layer::TopCopper, 2),
            make_trace_data(Layer::TopCopper, 3),
            make_trace_data(Layer::BottomCopper, 4),
        ];
        // min=1, max=4 → balance = 0.25
        let balance = compute_layer_balance(&traces);
        assert!(
            (balance - 0.25).abs() < 1e-10,
            "4:1 should be 0.25, got {balance}"
        );
    }

    // ====================================================================
    // Smoothness tests
    // ====================================================================

    #[test]
    fn test_smoothness_empty() {
        assert!((compute_smoothness(&[]) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_smoothness_no_bends() {
        // Single segment trace — no bends
        let traces = vec![TraceData {
            entity: Entity::from_raw(0),
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            )],
            layer: Layer::TopCopper,
            net_id: NetId::new(0),
        }];
        assert!(
            (compute_smoothness(&traces) - 1.0).abs() < 1e-10,
            "No bends should be perfect smoothness"
        );
    }

    #[test]
    fn test_smoothness_90_degree_bend() {
        // Two segments forming a 90° bend — a 45° multiple, so no penalty
        let traces = vec![TraceData {
            entity: Entity::from_raw(0),
            segments: vec![
                TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 0.0)),
                TraceSegment::new(Point::from_mm(10.0, 0.0), Point::from_mm(10.0, 10.0)),
            ],
            layer: Layer::TopCopper,
            net_id: NetId::new(0),
        }];
        let smoothness = compute_smoothness(&traces);
        assert!(
            (smoothness - 1.0).abs() < 1e-10,
            "90° bend is a 45° multiple → perfect smoothness, got {smoothness}"
        );
    }

    #[test]
    fn test_smoothness_45_degree_bend() {
        // Horizontal then 45° diagonal — 45° bend, a 45° multiple
        let traces = vec![TraceData {
            entity: Entity::from_raw(0),
            segments: vec![
                TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 0.0)),
                TraceSegment::new(Point::from_mm(10.0, 0.0), Point::from_mm(20.0, 10.0)),
            ],
            layer: Layer::TopCopper,
            net_id: NetId::new(0),
        }];
        let smoothness = compute_smoothness(&traces);
        assert!(
            (smoothness - 1.0).abs() < 1e-10,
            "45° bend should be perfect, got {smoothness}"
        );
    }

    #[test]
    fn test_smoothness_zero_length_segment_skipped() {
        // Zero-length segment should be skipped, not cause panic
        let traces = vec![TraceData {
            entity: Entity::from_raw(0),
            segments: vec![
                TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 0.0)),
                TraceSegment::new(Point::from_mm(10.0, 0.0), Point::from_mm(10.0, 0.0)), // zero-length
                TraceSegment::new(Point::from_mm(10.0, 0.0), Point::from_mm(10.0, 10.0)),
            ],
            layer: Layer::TopCopper,
            net_id: NetId::new(0),
        }];
        let smoothness = compute_smoothness(&traces);
        // Should not panic, and the valid bend (0° to 90°) should still be computed
        assert!(
            (0.0..=1.0).contains(&smoothness),
            "Smoothness with zero-length segment should be in [0,1], got {smoothness}"
        );
    }

    // ====================================================================
    // Composite formula tests
    // ====================================================================

    #[test]
    fn test_composite_all_zero() {
        let composite = compute_composite(
            Nm(0),         // length
            0,             // vias
            0,             // drc
            1.0,           // smoothness (perfect)
            0,             // crossings
            1.0,           // balance (perfect)
            100_000_000.0, // diagonal
            &ScoreWeights::default(),
        );
        assert!(
            composite.abs() < 1e-10,
            "Perfect board should have zero composite, got {composite}"
        );
    }

    #[test]
    fn test_composite_length_contributes() {
        let diagonal = 100_000_000.0; // 100mm
        let weights = ScoreWeights::default();

        let composite = compute_composite(
            Nm(100_000_000), // 100mm length = 1.0 normalized
            0,
            0,
            1.0,
            0,
            1.0,
            diagonal,
            &weights,
        );
        assert!(
            (composite - 1.0).abs() < 1e-10,
            "100mm length / 100mm diagonal = 1.0 contribution, got {composite}"
        );
    }

    #[test]
    fn test_composite_via_contributes() {
        let composite = compute_composite(
            Nm(0),
            5,
            0,
            1.0,
            0,
            1.0,
            100_000_000.0,
            &ScoreWeights::default(),
        );
        assert!(
            (composite - 5.0).abs() < 1e-10,
            "5 vias with weight 1.0 = 5.0, got {composite}"
        );
    }

    #[test]
    fn test_composite_drc_contributes() {
        let composite = compute_composite(
            Nm(0),
            0,
            3,
            1.0,
            0,
            1.0,
            100_000_000.0,
            &ScoreWeights::default(),
        );
        assert!(
            (composite - 3000.0).abs() < 1e-10,
            "3 DRC violations × 1000 = 3000, got {composite}"
        );
    }

    #[test]
    fn test_composite_lower_is_better() {
        let weights = ScoreWeights::default();
        let diagonal = 100_000_000.0;

        let good = compute_composite(Nm(50_000_000), 2, 0, 0.95, 0, 0.9, diagonal, &weights);
        let bad = compute_composite(Nm(200_000_000), 10, 5, 0.5, 3, 0.3, diagonal, &weights);
        assert!(
            good < bad,
            "Better routing should have lower composite: good={good} bad={bad}"
        );
    }

    // ====================================================================
    // RoutingScore JSON serialization
    // ====================================================================

    #[test]
    fn test_routing_score_serialization() {
        let score = RoutingScore {
            total_length: Nm::from_mm(100.0),
            via_count: 5,
            drc_violations: 0,
            smoothness: 0.95,
            crossings: 1,
            layer_balance: 0.8,
            composite: 42.5,
        };

        let json = serde_json::to_string(&score).expect("RoutingScore should serialize to JSON");
        assert!(
            json.contains("\"via_count\":5"),
            "JSON should contain via_count"
        );
        assert!(
            json.contains("\"smoothness\":0.95"),
            "JSON should contain smoothness"
        );
        assert!(
            json.contains("\"composite\":42.5"),
            "JSON should contain composite"
        );
    }

    // ====================================================================
    // score_board integration (unit-level with mock world)
    // ====================================================================

    #[test]
    fn test_score_board_empty_world() {
        let mut world = BoardWorld::new();
        world.set_board(
            "Empty".to_string(),
            (Nm::from_mm(50.0), Nm::from_mm(30.0)),
            2,
        );

        let rules = DesignRules::default();
        world.rebuild_spatial_index_from_library(&FootprintLibrary::new());
        let score = score_board(&mut world, &rules, &ScoreWeights::default());

        assert_eq!(score.total_length, Nm(0), "Empty board has zero length");
        assert_eq!(score.via_count, 0, "Empty board has zero vias");
        assert_eq!(score.crossings, 0, "Empty board has zero crossings");
        assert!(
            (score.smoothness - 1.0).abs() < 1e-10,
            "Empty board has perfect smoothness"
        );
        assert!(
            (score.layer_balance - 1.0).abs() < 1e-10,
            "Empty board has perfect balance"
        );
        assert!(
            score.composite.abs() < 1e-10,
            "Empty board has zero composite"
        );
    }

    #[test]
    fn test_score_board_with_traces() {
        let mut world = BoardWorld::new();
        world.set_board(
            "Test".to_string(),
            (Nm::from_mm(100.0), Nm::from_mm(100.0)),
            2,
        );

        // Spawn a simple trace
        let trace = Trace {
            segments: vec![
                TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 0.0)),
                TraceSegment::new(Point::from_mm(10.0, 0.0), Point::from_mm(10.0, 10.0)),
            ],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(1),
            locked: false,
            source: TraceSource::Manual,
        };
        world.spawn_entity(trace);

        let rules = DesignRules::default();
        world.rebuild_spatial_index_from_library(&FootprintLibrary::new());
        let score = score_board(&mut world, &rules, &ScoreWeights::default());

        // 10mm + 10mm = 20mm
        assert_eq!(score.total_length, Nm::from_mm(20.0));
        assert_eq!(score.via_count, 0);
        assert!(score.smoothness > 0.0 && score.smoothness <= 1.0);
        // Single layer → balance = 1.0
        assert!((score.layer_balance - 1.0).abs() < 1e-10);
        // Composite should be > 0 (we have length)
        assert!(score.composite > 0.0);
    }

    #[test]
    fn test_score_board_with_vias() {
        let mut world = BoardWorld::new();
        world.set_board(
            "Test".to_string(),
            (Nm::from_mm(50.0), Nm::from_mm(50.0)),
            2,
        );

        // Spawn vias
        let via1 = Via::new(Point::from_mm(10.0, 10.0), NetId::new(1));
        let via2 = Via::new(Point::from_mm(20.0, 20.0), NetId::new(1));
        world.spawn_entity(via1);
        world.spawn_entity(via2);

        let rules = DesignRules::default();
        world.rebuild_spatial_index_from_library(&FootprintLibrary::new());
        let score = score_board(&mut world, &rules, &ScoreWeights::default());

        assert_eq!(score.via_count, 2);
    }

    // ====================================================================
    // Crossing detection unit test
    // ====================================================================

    #[test]
    fn test_crossings_different_nets() {
        let mut world = BoardWorld::new();
        world.set_board(
            "Test".to_string(),
            (Nm::from_mm(50.0), Nm::from_mm(50.0)),
            2,
        );

        // Two crossing traces on same layer, different nets (X shape)
        let t1 = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 10.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(1),
            locked: false,
            source: TraceSource::Manual,
        };
        let t2 = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 10.0),
                Point::from_mm(10.0, 0.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(2),
            locked: false,
            source: TraceSource::Manual,
        };

        world.spawn_entity(t1);
        world.spawn_entity(t2);

        let rules = DesignRules::default();
        world.rebuild_spatial_index_from_library(&FootprintLibrary::new());
        let score = score_board(&mut world, &rules, &ScoreWeights::default());

        assert!(
            score.crossings >= 1,
            "Two crossing traces on same layer, different nets should have >=1 crossing, got {}",
            score.crossings
        );
    }

    #[test]
    fn test_crossings_same_net_not_counted() {
        let mut world = BoardWorld::new();
        world.set_board(
            "Test".to_string(),
            (Nm::from_mm(50.0), Nm::from_mm(50.0)),
            2,
        );

        // Two crossing traces on same layer, same net (junction)
        let t1 = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 10.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(1),
            locked: false,
            source: TraceSource::Manual,
        };
        let t2 = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 10.0),
                Point::from_mm(10.0, 0.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(1), // Same net
            locked: false,
            source: TraceSource::Manual,
        };

        world.spawn_entity(t1);
        world.spawn_entity(t2);

        let rules = DesignRules::default();
        world.rebuild_spatial_index_from_library(&FootprintLibrary::new());
        let score = score_board(&mut world, &rules, &ScoreWeights::default());

        assert_eq!(
            score.crossings, 0,
            "Same-net crossings should not be counted"
        );
    }

    #[test]
    fn test_crossings_different_layers_not_counted() {
        let mut world = BoardWorld::new();
        world.set_board(
            "Test".to_string(),
            (Nm::from_mm(50.0), Nm::from_mm(50.0)),
            2,
        );

        // Two crossing traces on different layers, different nets
        let t1 = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 10.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(1),
            locked: false,
            source: TraceSource::Manual,
        };
        let t2 = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 10.0),
                Point::from_mm(10.0, 0.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::BottomCopper, // Different layer
            net_id: NetId::new(2),
            locked: false,
            source: TraceSource::Manual,
        };

        world.spawn_entity(t1);
        world.spawn_entity(t2);

        let rules = DesignRules::default();
        world.rebuild_spatial_index_from_library(&FootprintLibrary::new());
        let score = score_board(&mut world, &rules, &ScoreWeights::default());

        assert_eq!(
            score.crossings, 0,
            "Different-layer crossings should not be counted"
        );
    }
}
