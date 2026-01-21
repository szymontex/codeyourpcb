//! DRC rule definitions and implementations.
//!
//! This module defines the [`DrcRule`] trait that all rules implement,
//! as well as the [`DesignRules`] configuration struct.

use cypcb_core::Nm;
use cypcb_world::BoardWorld;

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

/// Design rules configuration for DRC.
///
/// Contains minimum values for various design parameters. Use the
/// factory methods for manufacturer presets, or create custom rules.
///
/// # Examples
///
/// ```
/// use cypcb_drc::DesignRules;
/// use cypcb_core::Nm;
///
/// // Use a manufacturer preset
/// let jlcpcb = DesignRules::jlcpcb_2layer();
/// assert_eq!(jlcpcb.min_clearance, Nm::from_mm(0.15));
///
/// // Or create custom rules
/// let custom = DesignRules {
///     min_clearance: Nm::from_mm(0.2),
///     min_trace_width: Nm::from_mm(0.25),
///     min_drill_size: Nm::from_mm(0.4),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct DesignRules {
    /// Minimum clearance between copper features.
    pub min_clearance: Nm,
    /// Minimum trace width.
    pub min_trace_width: Nm,
    /// Minimum drill size (mechanical drilling).
    pub min_drill_size: Nm,
}

impl Default for DesignRules {
    /// Default rules use JLCPCB 2-layer values.
    fn default() -> Self {
        Self::jlcpcb_2layer()
    }
}

impl DesignRules {
    /// JLCPCB standard 2-layer board rules.
    ///
    /// Source: <https://jlcpcb.com/capabilities/pcb-capabilities>
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DesignRules;
    /// use cypcb_core::Nm;
    ///
    /// let rules = DesignRules::jlcpcb_2layer();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.15)); // 6 mil
    /// assert_eq!(rules.min_drill_size, Nm::from_mm(0.3)); // 0.3mm mechanical
    /// ```
    pub fn jlcpcb_2layer() -> Self {
        DesignRules {
            min_clearance: Nm::from_mm(0.15),     // 6 mil
            min_trace_width: Nm::from_mm(0.15),   // 6 mil
            min_drill_size: Nm::from_mm(0.3),     // 0.3mm mechanical
        }
    }

    /// JLCPCB 4-layer board rules (tighter tolerances available).
    ///
    /// Source: <https://jlcpcb.com/capabilities/pcb-capabilities>
    pub fn jlcpcb_4layer() -> Self {
        DesignRules {
            min_clearance: Nm::from_mm(0.1),      // 4 mil
            min_trace_width: Nm::from_mm(0.1),    // 4 mil
            min_drill_size: Nm::from_mm(0.2),     // 0.2mm
        }
    }

    /// PCBWay standard rules.
    ///
    /// Source: <https://www.pcbway.com/capabilities.html>
    pub fn pcbway_standard() -> Self {
        DesignRules {
            min_clearance: Nm::from_mm(0.15),     // Recommended 6 mil
            min_trace_width: Nm::from_mm(0.15),
            min_drill_size: Nm::from_mm(0.2),     // Mechanical
        }
    }

    /// Relaxed rules for prototyping (larger margins).
    ///
    /// Good for beginner designs or when using lower-quality fab houses.
    pub fn prototype() -> Self {
        DesignRules {
            min_clearance: Nm::from_mm(0.2),      // 8 mil
            min_trace_width: Nm::from_mm(0.25),   // 10 mil
            min_drill_size: Nm::from_mm(0.4),
        }
    }
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
    fn test_design_rules_jlcpcb_2layer() {
        let rules = DesignRules::jlcpcb_2layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.3));
    }

    #[test]
    fn test_design_rules_jlcpcb_4layer() {
        let rules = DesignRules::jlcpcb_4layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.1));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.1));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
    }

    #[test]
    fn test_design_rules_pcbway() {
        let rules = DesignRules::pcbway_standard();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
    }

    #[test]
    fn test_design_rules_prototype() {
        let rules = DesignRules::prototype();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.2));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.25));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.4));
    }

    #[test]
    fn test_design_rules_default() {
        let rules = DesignRules::default();
        // Default should be JLCPCB 2-layer
        assert_eq!(rules.min_clearance, DesignRules::jlcpcb_2layer().min_clearance);
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
