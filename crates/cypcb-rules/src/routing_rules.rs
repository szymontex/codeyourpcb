//! Routing rule trait for the A*-based autorouter.
//!
//! [`RoutingRuleSet`] is the primary interface between the design rules system
//! and the routing engine. It must be object-safe so it can be stored as
//! `dyn RoutingRuleSet` in the router.
//!
//! # Layer indices
//!
//! This module uses `u8` layer indices instead of importing `cypcb-world`'s
//! `Layer` enum, keeping `cypcb-rules` as a leaf crate. Convention:
//! - 0 = top copper
//! - 1 = inner-1
//! - N = bottom copper (where N = total_copper_layers - 1)

use crate::constraints::DesignConstraints;
use crate::signal_class::{SignalClass, SignalClassConstraints};
use cypcb_core::Nm;

/// Rule set interface for routing and DRC engines.
///
/// This trait is designed to be object-safe (`dyn RoutingRuleSet` compiles).
/// All methods take `&self` and return owned or reference types — no generics,
/// no `Self`-returning methods.
///
/// # For autorouter integration
///
/// The A* router uses:
/// - [`constraints_for_net`](RoutingRuleSet::constraints_for_net) to get per-net clearances and widths
/// - [`via_cost`](RoutingRuleSet::via_cost) to penalize layer transitions
/// - [`layer_change_cost`](RoutingRuleSet::layer_change_cost) to bias routing toward preferred layers
/// - [`clearance_between`](RoutingRuleSet::clearance_between) to check spacing between different nets
///
/// # Examples
///
/// ```
/// use cypcb_rules::{RoutingRuleSet, DesignConstraints, SignalClass, SignalClassConstraints};
/// use cypcb_core::Nm;
///
/// struct SimpleRules {
///     constraints: DesignConstraints,
/// }
///
/// impl RoutingRuleSet for SimpleRules {
///     fn constraints_for_net(&self, _net_id: u32) -> &DesignConstraints {
///         &self.constraints
///     }
///
///     fn constraints_for_class(&self, class: SignalClass) -> SignalClassConstraints {
///         class.default_constraints()
///     }
///
///     fn via_cost(&self, _from_layer: u8, _to_layer: u8) -> f64 {
///         1.0
///     }
///
///     fn layer_change_cost(&self, _layer: u8) -> f64 {
///         0.5
///     }
///
///     fn clearance_between(&self, _net_a: u32, _net_b: u32) -> Nm {
///         self.constraints.min_clearance
///     }
/// }
///
/// let rules = SimpleRules { constraints: DesignConstraints::default() };
/// let _: &dyn RoutingRuleSet = &rules; // object-safe
/// ```
pub trait RoutingRuleSet {
    /// Get the design constraints applicable to a specific net.
    ///
    /// For simple designs, this may return the same constraints for all nets.
    /// Advanced designs may have per-net overrides.
    fn constraints_for_net(&self, net_id: u32) -> &DesignConstraints;

    /// Get the signal-class-specific constraints.
    ///
    /// Returns an owned [`SignalClassConstraints`] because classes may be
    /// computed dynamically.
    fn constraints_for_class(&self, class: SignalClass) -> SignalClassConstraints;

    /// Cost of placing a via transitioning between two layers.
    ///
    /// Higher values discourage the router from adding vias. Typical range:
    /// - 0.5–2.0 for standard vias
    /// - 5.0+ for blind/buried vias (more expensive to fabricate)
    /// - `f64::INFINITY` to prohibit a transition entirely
    fn via_cost(&self, from_layer: u8, to_layer: u8) -> f64;

    /// Cost of routing on a specific layer.
    ///
    /// Used by the A* router to bias routing toward preferred layers.
    /// Lower values = preferred. Typical range: 0.1–2.0.
    fn layer_change_cost(&self, layer: u8) -> f64;

    /// Minimum clearance required between two specific nets.
    ///
    /// Allows net-pair-specific spacing rules (e.g. high-voltage isolation
    /// between power nets and signal nets).
    fn clearance_between(&self, net_a: u32, net_b: u32) -> Nm;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal implementation to verify the trait is object-safe and usable.
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
            span * 0.5
        }

        fn layer_change_cost(&self, layer: u8) -> f64 {
            if layer == 0 {
                0.1
            } else {
                1.0
            }
        }

        fn clearance_between(&self, _net_a: u32, _net_b: u32) -> Nm {
            self.base.min_clearance
        }
    }

    #[test]
    fn test_trait_object_safe() {
        let rules = TestRules::new();
        // This line proves object safety — if it compiles, dyn dispatch works.
        let dyn_rules: &dyn RoutingRuleSet = &rules;
        let c = dyn_rules.constraints_for_net(0);
        assert_eq!(c.min_clearance, DesignConstraints::default().min_clearance);
    }

    #[test]
    fn test_via_cost_scales_with_span() {
        let rules = TestRules::new();
        let cost_1 = rules.via_cost(0, 1);
        let cost_2 = rules.via_cost(0, 2);
        assert!(cost_2 > cost_1, "Multi-layer via should cost more");
    }

    #[test]
    fn test_layer_change_cost() {
        let rules = TestRules::new();
        assert!(rules.layer_change_cost(0) < rules.layer_change_cost(1));
    }

    #[test]
    fn test_clearance_between_returns_nm() {
        let rules = TestRules::new();
        let clearance = rules.clearance_between(1, 2);
        assert!(clearance.raw() > 0);
    }

    #[test]
    fn test_constraints_for_class() {
        let rules = TestRules::new();
        let dyn_rules: &dyn RoutingRuleSet = &rules;
        let power = dyn_rules.constraints_for_class(SignalClass::Power);
        let digital = dyn_rules.constraints_for_class(SignalClass::Digital);
        assert!(power.min_trace_width.raw() > digital.min_trace_width.raw());
    }

    #[test]
    fn test_dyn_dispatch_all_methods() {
        let rules = TestRules::new();
        let dyn_rules: &dyn RoutingRuleSet = &rules;

        // Exercise every trait method through dyn dispatch
        let _ = dyn_rules.constraints_for_net(42);
        let _ = dyn_rules.constraints_for_class(SignalClass::HighSpeed);
        let _ = dyn_rules.via_cost(0, 3);
        let _ = dyn_rules.layer_change_cost(1);
        let _ = dyn_rules.clearance_between(10, 20);
    }
}
