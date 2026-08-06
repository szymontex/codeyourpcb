//! PCB-aware cost function for A* pathfinding.
//!
//! [`RoutingCost`] wraps the design rules to provide movement costs that
//! account for via transitions, layer preference, diagonal routing, and
//! clearance concerns. The heuristic uses 3D octile distance for admissibility.

use crate::pathfinder::GridNode;
use cypcb_rules::RoutingRuleSet;

/// Square root of 2, cached for diagonal movement cost.
const SQRT2: f64 = std::f64::consts::SQRT_2;

/// PCB-aware cost calculator for A* search.
///
/// Holds a reference to the design rules and the net currently being routed.
/// All cost methods are pure functions of the node positions and rules — the
/// grid itself is not consulted here (that's the pathfinder's job when
/// generating successors).
pub struct RoutingCost<'a> {
    /// Design rules for via cost, layer preference, clearance values.
    rules: &'a dyn RoutingRuleSet,
    /// The net being routed — used for net-specific constraint lookup.
    net_id: u32,
    /// Precomputed minimum via cost across all layer pairs (for heuristic).
    min_via_cost: f64,
    /// Config-driven multiplier for via cost.
    via_cost_multiplier: f64,
    /// Layer preference bias: -1.0=bottom-heavy, 0.0=balanced, 1.0=top-heavy.
    layer_preference: f64,
}

impl<'a> RoutingCost<'a> {
    /// Create a new cost calculator for routing a specific net.
    pub fn new(
        rules: &'a dyn RoutingRuleSet,
        net_id: u32,
        via_cost_multiplier: f64,
        layer_preference: f64,
    ) -> Self {
        // Precompute the minimum via cost for admissible heuristic.
        // Sample a few common layer pairs.
        let mut min_via = f64::MAX;
        for from in 0..4u8 {
            for to in 0..4u8 {
                if from != to {
                    let c = rules.via_cost(from, to);
                    if c < min_via {
                        min_via = c;
                    }
                }
            }
        }
        if min_via == f64::MAX {
            min_via = 1.0; // fallback
        }

        Self {
            rules,
            net_id,
            min_via_cost: min_via * via_cost_multiplier,
            via_cost_multiplier,
            layer_preference,
        }
    }

    /// Cost of moving from one grid node to an adjacent node.
    ///
    /// Components:
    /// - **Base movement**: 1.0 for cardinal (N/S/E/W), √2 for diagonal
    /// - **Layer change**: via cost from rules if layers differ
    /// - **Layer preference**: small bias from `layer_change_cost()` on destination layer
    ///
    /// The pathfinder guarantees `from` and `to` are adjacent (1 cell apart
    /// in each axis, or same position with a layer change).
    pub fn neighbor_cost(&self, from: GridNode, to: GridNode) -> f64 {
        let dx = (from.0 as i32 - to.0 as i32).unsigned_abs();
        let dy = (from.1 as i32 - to.1 as i32).unsigned_abs();

        let mut cost = if from.2 != to.2 {
            // Layer transition (via) — position is same, only layer changes
            self.rules.via_cost(from.2, to.2) * self.via_cost_multiplier
        } else if dx == 1 && dy == 1 {
            // Diagonal move
            SQRT2
        } else {
            // Cardinal move (dx + dy == 1)
            1.0
        };

        // Add a layer-preference bias on the destination layer.
        // When layer_preference > 0 (top-heavy), top layer (0) cost is reduced,
        // bottom layer cost is increased. When < 0 (bottom-heavy), reversed.
        let layer_bias = if self.layer_preference.abs() < f64::EPSILON {
            // Balanced: use the old fixed bias
            self.rules.layer_change_cost(to.2) * 0.1
        } else {
            // Asymmetric: layer 0 = top, others = bottom
            let direction = if to.2 == 0 { -1.0 } else { 1.0 };
            self.rules.layer_change_cost(to.2) * 0.1 * (1.0 + self.layer_preference * direction)
        };
        cost += layer_bias;

        cost
    }

    /// Admissible heuristic: 3D octile distance.
    ///
    /// On each layer: `max(|dx|, |dy|) + (√2 - 1) * min(|dx|, |dy|)`
    /// Plus minimum via cost per layer transition needed.
    ///
    /// This never overestimates the true cost, so A* remains optimal.
    pub fn heuristic(&self, current: GridNode, goal: GridNode) -> f64 {
        let dx = (current.0 as i32 - goal.0 as i32).unsigned_abs() as f64;
        let dy = (current.1 as i32 - goal.1 as i32).unsigned_abs() as f64;
        let layer_diff = (current.2 as i32 - goal.2 as i32).unsigned_abs() as f64;

        let dmin = dx.min(dy);
        let dmax = dx.max(dy);

        // 2D octile distance + via cost for layer changes
        dmax + (SQRT2 - 1.0) * dmin + self.min_via_cost * layer_diff
    }

    /// Get the net ID this cost function is configured for.
    pub fn net_id(&self) -> u32 {
        self.net_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathfinder::GridNode;
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
    fn cardinal_move_costs_one() {
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0);
        let from: GridNode = (5, 5, 0);
        let to: GridNode = (6, 5, 0);
        let c = cost.neighbor_cost(from, to);
        // Base 1.0 + small layer preference
        assert!((1.0..1.5).contains(&c), "Cardinal cost: {c}");
    }

    #[test]
    fn diagonal_move_costs_sqrt2() {
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0);
        let from: GridNode = (5, 5, 0);
        let to: GridNode = (6, 6, 0);
        let c = cost.neighbor_cost(from, to);
        assert!((SQRT2..SQRT2 + 0.5).contains(&c), "Diagonal cost: {c}");
    }

    #[test]
    fn via_transition_costs_more_than_movement() {
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0);
        let cardinal = cost.neighbor_cost((5, 5, 0), (6, 5, 0));
        let via = cost.neighbor_cost((5, 5, 0), (5, 5, 1));
        assert!(
            via > cardinal,
            "Via cost {via} should exceed cardinal {cardinal}"
        );
    }

    #[test]
    fn heuristic_is_admissible_straight_line() {
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0);
        // Straight 10-cell path on same layer
        let h = cost.heuristic((0, 0, 0), (10, 0, 0));
        // Heuristic should be <= actual path cost (10 cardinal moves = 10.0)
        assert!(h <= 10.0 + 1e-9, "Heuristic {h} should be <= 10.0");
    }

    #[test]
    fn heuristic_diagonal_path() {
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0);
        // Diagonal 5-cell path
        let h = cost.heuristic((0, 0, 0), (5, 5, 0));
        // Pure diagonal: 5 * √2 ≈ 7.07
        let expected = 5.0 * SQRT2;
        assert!(
            (h - expected).abs() < 1e-9,
            "Heuristic {h} should be {expected}"
        );
    }

    #[test]
    fn heuristic_includes_layer_cost() {
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0);
        let same_layer = cost.heuristic((0, 0, 0), (5, 0, 0));
        let diff_layer = cost.heuristic((0, 0, 0), (5, 0, 1));
        assert!(
            diff_layer > same_layer,
            "Cross-layer heuristic {diff_layer} should exceed same-layer {same_layer}"
        );
    }

    #[test]
    fn straight_path_cheaper_than_zigzag() {
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0);

        // Straight path: (0,0) -> (1,0) -> (2,0) -> (3,0) -> (4,0)
        let straight_cost: f64 = [(0u16, 0u16), (1, 0), (2, 0), (3, 0)]
            .windows(2)
            .map(|w| cost.neighbor_cost((w[0].0, w[0].1, 0), (w[1].0, w[1].1, 0)))
            .sum();

        // Zigzag path: (0,0) -> (1,1) -> (2,0) -> (3,1) -> (4,0)
        let zigzag_cost: f64 = [(0u16, 0u16), (1, 1), (2, 0), (3, 1)]
            .windows(2)
            .map(|w| cost.neighbor_cost((w[0].0, w[0].1, 0), (w[1].0, w[1].1, 0)))
            .sum();

        assert!(
            straight_cost < zigzag_cost,
            "Straight {straight_cost} should be cheaper than zigzag {zigzag_cost}"
        );
    }
}
