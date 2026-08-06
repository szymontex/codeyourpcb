//! DRC rule definitions and implementations.
//!
//! This module defines the [`DrcRule`] trait that all rules implement.
//! Design rules configuration is defined in the [`presets`](crate::presets) module.

pub mod annular_ring;
pub mod assertion;
pub mod clearance;
pub mod connectivity;
pub mod courtyard_clearance;
pub mod drill_size;
pub mod edge_clearance;
pub mod hole_to_hole;
pub mod silk_clearance;
pub mod solder_mask_bridge;
pub mod trace_current;
pub mod trace_width;
pub mod via_diameter;
pub mod via_drill;

use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

pub use annular_ring::AnnularRingRule;
pub use assertion::AssertionRule;
pub use clearance::ClearanceRule;
pub use connectivity::UnconnectedPinRule;
pub use courtyard_clearance::CourtyardClearanceRule;
pub use drill_size::MinDrillSizeRule;
pub use edge_clearance::EdgeClearanceRule;
pub use hole_to_hole::HoleToHoleRule;
pub use silk_clearance::SilkClearanceRule;
pub use solder_mask_bridge::SolderMaskBridgeRule;
pub use trace_current::TraceCurrentRule;
pub use trace_width::MinTraceWidthRule;
pub use via_diameter::ViaDiameterRule;
pub use via_drill::ViaDrillRule;

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
    /// * `world` - The board world to check (mutable for ECS queries)
    /// * `rules` - Design rules configuration
    ///
    /// Note: Takes `&mut BoardWorld` because bevy_ecs queries need to
    /// initialize their cache. No actual board data is modified.
    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation>;
}

/// Rule for checking components against keepout zones.
///
/// This rule checks if any component's position falls within a keepout zone.
/// Note: Currently only checks center point - a more complete implementation
/// would check the entire component footprint bounds.
///
/// # Examples
///
/// ```rust,ignore
/// use cypcb_drc::rules::KeepoutRule;
/// use cypcb_drc::rules::DrcRule;
///
/// let rule = KeepoutRule;
/// let violations = rule.check(&world, &rules);
/// for v in violations {
///     println!("Keepout violation: {}", v.message);
/// }
/// ```
pub struct KeepoutRule;

impl DrcRule for KeepoutRule {
    fn name(&self) -> &'static str {
        "keepout"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();

        // Collect all keepout zones
        let zones: Vec<_> = world
            .zones()
            .into_iter()
            .filter(|(_, zone)| zone.is_keepout())
            .collect();

        // If no keepout zones, skip
        if zones.is_empty() {
            return violations;
        }

        // Collect all components
        let components = world.components();

        // Check each component against keepout zones
        for (entity, refdes, position) in components {
            for (zone_entity, zone) in &zones {
                // Check if component center is inside zone
                if zone.contains(position.0) {
                    violations.push(DrcViolation::keepout(
                        entity,
                        *zone_entity,
                        refdes.as_str(),
                        zone.name.as_deref(),
                        position.0,
                    ));
                }
            }
        }

        violations
    }
}

/// Rotate a point about the origin by `degrees`, clockwise-positive to match
/// the `Rotation` component.
///
/// Placement geometry needs this in three rules; it lives here so they cannot
/// drift apart.
pub(crate) fn rotate_point(p: cypcb_core::Point, degrees: f64) -> cypcb_core::Point {
    use cypcb_core::{Nm, Point};
    if degrees.abs() < 0.001 {
        return p;
    }
    let rad = degrees.to_radians();
    let (sin, cos) = rad.sin_cos();
    let x = p.x.raw() as f64;
    let y = p.y.raw() as f64;
    Point::new(
        Nm((x * cos - y * sin).round() as i64),
        Nm((x * sin + y * cos).round() as i64),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ViolationKind;
    use cypcb_core::{Point, Rect};
    use cypcb_world::components::zone::ZoneKind;
    use cypcb_world::components::{
        FootprintRef, NetConnections, Position, RefDes, Rotation, Value, Zone,
    };

    #[test]
    fn test_trait_object_safe() {
        // Verify that DrcRule can be used as a trait object
        fn _assert_object_safe(_: &dyn DrcRule) {}
    }

    #[test]
    fn test_rule_names() {
        assert_eq!(ClearanceRule.name(), "clearance");
        assert_eq!(MinDrillSizeRule.name(), "min-drill-size");
        assert_eq!(MinTraceWidthRule.name(), "min-trace-width");
        assert_eq!(UnconnectedPinRule.name(), "unconnected-pin");
        assert_eq!(KeepoutRule.name(), "keepout");
        assert_eq!(EdgeClearanceRule.name(), "edge-clearance");
        assert_eq!(AnnularRingRule.name(), "annular-ring");
        assert_eq!(HoleToHoleRule.name(), "hole-to-hole");
        assert_eq!(ViaDiameterRule.name(), "via-diameter");
        assert_eq!(CourtyardClearanceRule.name(), "courtyard-clearance");
        assert_eq!(SolderMaskBridgeRule.name(), "solder-mask-bridge");
        assert_eq!(SilkClearanceRule.name(), "silk-clearance");
    }

    #[test]
    fn test_rule_check_empty_world() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();

        // All rules should return empty on empty world
        assert!(ClearanceRule.check(&mut world, &rules).is_empty());
        assert!(MinDrillSizeRule.check(&mut world, &rules).is_empty());
        assert!(MinTraceWidthRule.check(&mut world, &rules).is_empty());
        assert!(UnconnectedPinRule.check(&mut world, &rules).is_empty());
        assert!(KeepoutRule.check(&mut world, &rules).is_empty());
        assert!(EdgeClearanceRule.check(&mut world, &rules).is_empty());
        assert!(AnnularRingRule.check(&mut world, &rules).is_empty());
        assert!(HoleToHoleRule.check(&mut world, &rules).is_empty());
        assert!(ViaDiameterRule.check(&mut world, &rules).is_empty());
        assert!(CourtyardClearanceRule.check(&mut world, &rules).is_empty());
        assert!(SolderMaskBridgeRule.check(&mut world, &rules).is_empty());
        assert!(SilkClearanceRule.check(&mut world, &rules).is_empty());
    }

    #[test]
    fn test_rule_trait_object_vec() {
        // Verify rules can be collected into a Vec<Box<dyn DrcRule>>
        let rules: Vec<Box<dyn DrcRule>> = vec![
            Box::new(ClearanceRule),
            Box::new(MinDrillSizeRule),
            Box::new(MinTraceWidthRule),
            Box::new(UnconnectedPinRule),
            Box::new(KeepoutRule),
            Box::new(EdgeClearanceRule),
            Box::new(AnnularRingRule),
            Box::new(HoleToHoleRule),
            Box::new(ViaDiameterRule),
            Box::new(CourtyardClearanceRule),
            Box::new(SolderMaskBridgeRule),
            Box::new(SilkClearanceRule),
        ];
        assert_eq!(rules.len(), 12);
    }

    #[test]
    fn test_keepout_rule_detects_violation() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();

        // Create a keepout zone
        let zone = Zone {
            bounds: Rect::new(Point::from_mm(10.0, 10.0), Point::from_mm(20.0, 20.0)),
            kind: ZoneKind::Keepout,
            layer_mask: 0xFFFFFFFF,
            name: Some("test_zone".to_string()),
            net: None,
        };
        world.ecs_mut().spawn(zone);

        // Create a component inside the zone
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(15.0, 15.0), // Inside zone
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Run the rule
        let violations = KeepoutRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 1, "expected 1 violation");
        assert_eq!(violations[0].kind, ViolationKind::KeepoutViolation);
        assert!(violations[0].message.contains("R1"));
        assert!(violations[0].message.contains("test_zone"));
    }

    #[test]
    fn test_keepout_rule_no_violation_outside() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();

        // Create a keepout zone
        let zone = Zone {
            bounds: Rect::new(Point::from_mm(10.0, 10.0), Point::from_mm(20.0, 20.0)),
            kind: ZoneKind::Keepout,
            layer_mask: 0xFFFFFFFF,
            name: Some("test_zone".to_string()),
            net: None,
        };
        world.ecs_mut().spawn(zone);

        // Create a component outside the zone
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(5.0, 5.0), // Outside zone
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Run the rule
        let violations = KeepoutRule.check(&mut world, &rules);
        assert!(violations.is_empty(), "expected no violations");
    }

    #[test]
    fn test_keepout_rule_ignores_copper_pour_zones() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();

        // Create a copper pour zone (not keepout)
        let zone = Zone {
            bounds: Rect::new(Point::from_mm(0.0, 0.0), Point::from_mm(50.0, 50.0)),
            kind: ZoneKind::CopperPour,
            layer_mask: 0xFFFFFFFF,
            name: Some("gnd_pour".to_string()),
            net: None,
        };
        world.ecs_mut().spawn(zone);

        // Create a component inside the copper pour zone
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(15.0, 15.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Run the rule - should not detect violation (copper pour is not keepout)
        let violations = KeepoutRule.check(&mut world, &rules);
        assert!(
            violations.is_empty(),
            "copper pour zones should not trigger keepout violations"
        );
    }

    #[test]
    fn test_keepout_rule_multiple_components() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();

        // Create a keepout zone
        let zone = Zone {
            bounds: Rect::new(Point::from_mm(10.0, 10.0), Point::from_mm(20.0, 20.0)),
            kind: ZoneKind::Keepout,
            layer_mask: 0xFFFFFFFF,
            name: None,
            net: None,
        };
        world.ecs_mut().spawn(zone);

        // Component inside
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(15.0, 15.0), // Inside
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Component outside
        world.spawn_component(
            RefDes::new("R2"),
            Value::new("10k"),
            Position::from_mm(5.0, 5.0), // Outside
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Another component inside
        world.spawn_component(
            RefDes::new("R3"),
            Value::new("10k"),
            Position::from_mm(12.0, 18.0), // Inside
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Run the rule
        let violations = KeepoutRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 2, "expected 2 violations (R1 and R3)");

        // Verify both violations are for the right components
        let refdes_list: Vec<_> = violations.iter().map(|v| v.message.clone()).collect();
        assert!(refdes_list.iter().any(|m| m.contains("R1")));
        assert!(refdes_list.iter().any(|m| m.contains("R3")));
        assert!(!refdes_list.iter().any(|m| m.contains("R2")));
    }
}
