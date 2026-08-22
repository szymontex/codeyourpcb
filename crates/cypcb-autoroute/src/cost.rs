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
/// Holds the net being routed and the answers the design rules gave for this
/// board, read once at construction. It borrows nothing: every question the
/// search asks is a table lookup, and the rule set is consulted only while
/// this is being built.
/// All cost methods are pure functions of the node positions and rules — the
/// grid itself is not consulted here (that's the pathfinder's job when
/// generating successors).
pub struct RoutingCost {
    /// The net being routed — used for net-specific constraint lookup.
    net_id: u32,
    /// Precomputed minimum via cost across all layer pairs (for heuristic).
    min_via_cost: f64,
    /// What a layer change from one layer to another costs, already
    /// multiplied: `via[from * layers + to]`.
    ///
    /// `neighbor_cost` runs once per neighbour - eight to eleven times per
    /// node expansion, millions of times per board - and it used to call
    /// `rules.via_cost` and `rules.layer_change_cost` through a trait object
    /// every time, for an answer that depends on nothing but two small
    /// integers. A four-layer board has sixteen of those answers; they are
    /// computed once here.
    via: Vec<f64>,
    /// The layer-preference bias per destination layer, already scaled.
    bias: Vec<f64>,
    /// How many layers the tables above are sized for.
    layers: usize,
    /// What the heuristic is multiplied by. 1.0 keeps A* optimal.
    heuristic_weight: f64,
}

impl RoutingCost {
    /// Create a new cost calculator for routing a specific net.
    pub fn new(
        rules: &dyn RoutingRuleSet,
        net_id: u32,
        via_cost_multiplier: f64,
        layer_preference: f64,
        layer_count: u8,
    ) -> Self {
        Self::weighted(
            rules,
            net_id,
            via_cost_multiplier,
            layer_preference,
            layer_count,
            1.0,
        )
    }

    /// The same, with the search's estimate scaled by `heuristic_weight`.
    pub fn weighted(
        rules: &dyn RoutingRuleSet,
        net_id: u32,
        via_cost_multiplier: f64,
        layer_preference: f64,
        layer_count: u8,
        heuristic_weight: f64,
    ) -> Self {
        // At least two, so a caller that knows nothing about the board still
        // gets a table with a layer change in it.
        let layers = (layer_count as usize).max(2);

        let mut via = vec![0.0; layers * layers];
        let mut min_via = f64::MAX;
        for from in 0..layers {
            for to in 0..layers {
                let cost = rules.via_cost(from as u8, to as u8) * via_cost_multiplier;
                via[from * layers + to] = cost;
                if from != to && cost < min_via {
                    min_via = cost;
                }
            }
        }
        if min_via == f64::MAX {
            min_via = via_cost_multiplier; // fallback
        }

        // The bias a destination layer carries, with the asymmetry already
        // applied: layer 0 is the top, everything else is towards the bottom.
        let bias = (0..layers)
            .map(|layer| {
                let base = rules.layer_change_cost(layer as u8) * 0.1;
                if layer_preference.abs() < f64::EPSILON {
                    base
                } else {
                    let direction = if layer == 0 { -1.0 } else { 1.0 };
                    base * (1.0 + layer_preference * direction)
                }
            })
            .collect();

        Self {
            net_id,
            min_via_cost: min_via,
            via,
            bias,
            layers,
            heuristic_weight,
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

        let cost = if from.2 != to.2 {
            // Layer transition (via) — position is same, only layer changes
            self.via_cost(from.2, to.2)
        } else if dx == 1 && dy == 1 {
            // Diagonal move
            SQRT2
        } else {
            // Cardinal move (dx + dy == 1)
            1.0
        };

        cost + self.layer_bias(to.2)
    }

    /// What a layer change costs, from the table built for this board.
    ///
    /// A layer the table does not cover cannot be reached by the search that
    /// built it; answering with the largest cost in the table keeps such a
    /// move from looking cheap.
    #[inline]
    fn via_cost(&self, from: u8, to: u8) -> f64 {
        let (from, to) = (from as usize, to as usize);
        if from < self.layers && to < self.layers {
            self.via[from * self.layers + to]
        } else {
            self.min_via_cost
        }
    }

    /// The layer-preference bias for a destination layer.
    #[inline]
    fn layer_bias(&self, layer: u8) -> f64 {
        self.bias.get(layer as usize).copied().unwrap_or(0.0)
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
        let estimate = dmax + (SQRT2 - 1.0) * dmin + self.min_via_cost * layer_diff;
        estimate * self.heuristic_weight
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
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0, 4);
        let from: GridNode = (5, 5, 0);
        let to: GridNode = (6, 5, 0);
        let c = cost.neighbor_cost(from, to);
        // Base 1.0 + small layer preference
        assert!((1.0..1.5).contains(&c), "Cardinal cost: {c}");
    }

    #[test]
    fn diagonal_move_costs_sqrt2() {
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0, 4);
        let from: GridNode = (5, 5, 0);
        let to: GridNode = (6, 6, 0);
        let c = cost.neighbor_cost(from, to);
        assert!((SQRT2..SQRT2 + 0.5).contains(&c), "Diagonal cost: {c}");
    }

    #[test]
    fn via_transition_costs_more_than_movement() {
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0, 4);
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
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0, 4);
        // Straight 10-cell path on same layer
        let h = cost.heuristic((0, 0, 0), (10, 0, 0));
        // Heuristic should be <= actual path cost (10 cardinal moves = 10.0)
        assert!(h <= 10.0 + 1e-9, "Heuristic {h} should be <= 10.0");
    }

    #[test]
    fn heuristic_diagonal_path() {
        let rules = TestRules::new();
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0, 4);
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
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0, 4);
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
        let cost = RoutingCost::new(&rules, 0, 1.0, 0.0, 4);

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
