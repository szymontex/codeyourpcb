//! DRC rule definitions and implementations.
//!
//! This module defines the [`DrcRule`] trait that all rules implement.
//! Design rules configuration is defined in the [`presets`](crate::presets) module.

use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// A single DRC rule that can be executed against a board.
///
/// Rules are implemented as structs that hold no state. Configuration
/// comes from the [`DesignRules`] struct passed to `check()`.
///
/// # Object Safety
///
/// This trait is designed to be object-safe, allowing rules to be
/// stored in a `Vec<Box<dyn DrcRule>>` for flexible rule composition.
///
/// # Examples
///
/// ```rust,ignore
/// impl DrcRule for ClearanceRule {
///     fn name(&self) -> &'static str {
///         "clearance"
///     }
///
///     fn check(&self, world: &BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
///         // Implementation...
///         vec![]
///     }
/// }
/// ```
pub trait DrcRule: Send + Sync {
    /// Rule identifier for error messages and filtering.
    fn name(&self) -> &'static str;

    /// Execute the rule check against the board world.
    ///
    /// Returns a list of violations (empty if rule passes).
    ///
    /// # Arguments
    ///
    /// * `world` - The board world to check
    /// * `rules` - Design rules configuration
    fn check(&self, world: &BoardWorld, rules: &DesignRules) -> Vec<DrcViolation>;
}

/// Placeholder rule for clearance checking.
///
/// Will be fully implemented in a later plan.
pub struct ClearanceRule;

impl DrcRule for ClearanceRule {
    fn name(&self) -> &'static str {
        "clearance"
    }

    fn check(&self, _world: &BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        // TODO: Implement clearance checking using spatial index
        Vec::new()
    }
}

/// Placeholder rule for minimum drill size checking.
///
/// Will be fully implemented in a later plan.
pub struct MinDrillSizeRule;

impl DrcRule for MinDrillSizeRule {
    fn name(&self) -> &'static str {
        "drill-size"
    }

    fn check(&self, _world: &BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        // TODO: Implement drill size checking
        Vec::new()
    }
}

/// Placeholder rule for unconnected pin detection.
///
/// Will be fully implemented in a later plan.
pub struct UnconnectedPinRule;

impl DrcRule for UnconnectedPinRule {
    fn name(&self) -> &'static str {
        "unconnected-pin"
    }

    fn check(&self, _world: &BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        // TODO: Implement unconnected pin detection
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_object_safe() {
        // Verify that DrcRule can be used as a trait object
        fn _assert_object_safe(_: &dyn DrcRule) {}
    }

    #[test]
    fn test_rule_names() {
        assert_eq!(ClearanceRule.name(), "clearance");
        assert_eq!(MinDrillSizeRule.name(), "drill-size");
        assert_eq!(UnconnectedPinRule.name(), "unconnected-pin");
    }

    #[test]
    fn test_rule_check_empty_world() {
        let world = BoardWorld::new();
        let rules = DesignRules::default();

        // All placeholder rules should return empty
        assert!(ClearanceRule.check(&world, &rules).is_empty());
        assert!(MinDrillSizeRule.check(&world, &rules).is_empty());
        assert!(UnconnectedPinRule.check(&world, &rules).is_empty());
    }

    #[test]
    fn test_rule_trait_object_vec() {
        // Verify rules can be collected into a Vec<Box<dyn DrcRule>>
        let rules: Vec<Box<dyn DrcRule>> = vec![
            Box::new(ClearanceRule),
            Box::new(MinDrillSizeRule),
            Box::new(UnconnectedPinRule),
        ];
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].name(), "clearance");
        assert_eq!(rules[1].name(), "drill-size");
        assert_eq!(rules[2].name(), "unconnected-pin");
    }
}
