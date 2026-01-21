//! Minimum drill size rule (DRC-03).
//!
//! Validates that all through-hole pads have drill holes meeting the minimum size
//! specified by the manufacturer's design rules.

use cypcb_world::BoardWorld;
use cypcb_world::components::{FootprintRef, Position, RefDes};
use cypcb_world::footprint::FootprintLibrary;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;
use super::DrcRule;

/// Rule that checks all drill holes meet minimum size.
///
/// Iterates through all components, looks up their footprints, and checks
/// each through-hole pad's drill size against `min_drill_size` from DesignRules.
///
/// SMD pads (no drill) are automatically exempt from this check.
///
/// # Examples
///
/// ```rust,ignore
/// use cypcb_drc::rules::{MinDrillSizeRule, DrcRule};
/// use cypcb_drc::presets::DesignRules;
///
/// let rule = MinDrillSizeRule;
/// let mut world = BoardWorld::new();
/// // ... add components ...
/// let rules = DesignRules::jlcpcb_2layer(); // min_drill = 0.3mm
/// let violations = rule.check(&mut world, &rules);
/// ```
pub struct MinDrillSizeRule;

impl DrcRule for MinDrillSizeRule {
    fn name(&self) -> &'static str {
        "min-drill-size"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_drill = rules.min_drill_size;
        let lib = FootprintLibrary::new();

        // Collect components first to avoid borrow issues
        let components: Vec<_> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &RefDes, &FootprintRef, &Position)>();
            query
                .iter(ecs)
                .map(|(e, r, f, p)| (e, r.clone(), f.clone(), *p))
                .collect()
        };

        for (entity, refdes, footprint_ref, position) in components {
            // Look up footprint in library
            let Some(footprint) = lib.get(footprint_ref.as_str()) else {
                continue; // Unknown footprint - skip (already caught by sync)
            };

            // Check each through-hole pad
            for pad in &footprint.pads {
                if let Some(drill) = pad.drill {
                    if drill < min_drill {
                        // Calculate pad's absolute position
                        let pad_location = cypcb_core::Point::new(
                            cypcb_core::Nm(position.0.x.0 + pad.position.x.0),
                            cypcb_core::Nm(position.0.y.0 + pad.position.y.0),
                        );
                        violations.push(DrcViolation::drill_size(
                            entity,
                            drill,
                            min_drill,
                            pad_location,
                        ).with_pad_info(&refdes.as_str(), &pad.number));
                    }
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::{Nm, Point};
    use cypcb_world::components::{Rotation, Value, NetConnections};

    #[test]
    fn test_rule_name() {
        assert_eq!(MinDrillSizeRule.name(), "min-drill-size");
    }

    #[test]
    fn test_small_drill_violation() {
        // DIP-8 has 1.0mm drills, test with 1.5mm minimum
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("U1"),
            Value::new("ATmega328P"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("DIP-8"),
            NetConnections::new(),
        );

        let rules = DesignRules {
            min_drill_size: Nm::from_mm(1.5), // Larger than DIP-8's 1.0mm drills
            ..DesignRules::default()
        };

        let violations = MinDrillSizeRule.check(&mut world, &rules);
        // DIP-8 has 8 pins with 1.0mm drills, all should violate
        assert_eq!(violations.len(), 8);
        for v in &violations {
            assert_eq!(v.kind, crate::ViolationKind::DrillSize);
        }
    }

    #[test]
    fn test_adequate_drill_no_violation() {
        // DIP-8 has 1.0mm drills, test with 0.8mm minimum
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("U1"),
            Value::new("ATmega328P"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("DIP-8"),
            NetConnections::new(),
        );

        let rules = DesignRules {
            min_drill_size: Nm::from_mm(0.8), // Smaller than DIP-8's 1.0mm drills
            ..DesignRules::default()
        };

        let violations = MinDrillSizeRule.check(&mut world, &rules);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_smd_pad_no_violation() {
        // 0402 has no drills (SMD only)
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let rules = DesignRules {
            min_drill_size: Nm::from_mm(1.0), // Very large minimum
            ..DesignRules::default()
        };

        let violations = MinDrillSizeRule.check(&mut world, &rules);
        assert!(violations.is_empty(), "SMD pads should not trigger drill violations");
    }

    #[test]
    fn test_empty_world_no_violations() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();
        let violations = MinDrillSizeRule.check(&mut world, &rules);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_unknown_footprint_skipped() {
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("X1"),
            Value::new("Unknown"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("NONEXISTENT-FOOTPRINT"),
            NetConnections::new(),
        );

        let rules = DesignRules::default();
        let violations = MinDrillSizeRule.check(&mut world, &rules);
        assert!(violations.is_empty(), "Unknown footprints should be skipped");
    }

    #[test]
    fn test_violation_has_correct_location() {
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("U1"),
            Value::new("IC"),
            Position::from_mm(100.0, 50.0),
            Rotation::ZERO,
            FootprintRef::new("DIP-8"),
            NetConnections::new(),
        );

        let rules = DesignRules {
            min_drill_size: Nm::from_mm(1.5),
            ..DesignRules::default()
        };

        let violations = MinDrillSizeRule.check(&mut world, &rules);
        assert!(!violations.is_empty());

        // Check that all violations have locations near the component position
        // (within reasonable footprint bounds)
        for v in &violations {
            assert!(v.location.x.0 > Nm::from_mm(90.0).0);
            assert!(v.location.x.0 < Nm::from_mm(110.0).0);
        }
    }
}
