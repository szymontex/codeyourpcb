//! Annular ring rule.
//!
//! Validates that through-hole pads have sufficient copper ring width
//! around the drill hole. The annular ring is:
//! `(pad_diameter - drill_diameter) / 2`.
//!
//! For non-circular pads, the smaller of width/height is used as the
//! effective pad diameter (worst-case check).

use cypcb_core::{Nm, Point};
use cypcb_world::components::{FootprintRef, Position, RefDes};
use cypcb_world::BoardWorld;

use super::DrcRule;
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// Rule that checks annular ring width around drill holes.
///
/// For each through-hole pad, verifies that the copper ring around the
/// drill hole meets the minimum annular ring width from DesignRules.
///
/// SMD pads (no drill) are automatically exempt.
///
/// # Examples
///
/// ```rust,ignore
/// use cypcb_drc::rules::{AnnularRingRule, DrcRule};
/// use cypcb_drc::presets::DesignRules;
///
/// let rule = AnnularRingRule;
/// let mut world = BoardWorld::new();
/// // ... add components with through-hole pads ...
/// let rules = DesignRules::jlcpcb_2layer(); // 0.15mm min annular ring
/// let violations = rule.check(&mut world, &rules);
/// ```
pub struct AnnularRingRule;

impl DrcRule for AnnularRingRule {
    fn name(&self) -> &'static str {
        "annular-ring"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_ring = rules.min_annular_ring;

        // Collect components first to avoid borrow issues with ECS
        let components: Vec<_> = {
            let ecs = world.ecs_mut();
            let mut query =
                ecs.query::<(bevy_ecs::entity::Entity, &RefDes, &FootprintRef, &Position)>();
            query
                .iter(ecs)
                .map(|(e, r, f, p)| (e, r.clone(), f.clone(), *p))
                .collect()
        };

        // The board carries the table it was synced with, including any footprint
        // the source defined inline; building a fresh one here would see built-ins only.
        let lib = world.footprints();
        for (entity, refdes, footprint_ref, position) in components {
            let Some(footprint) = lib.get(footprint_ref.as_str()) else {
                continue; // Unknown footprint — skip
            };

            for pad in &footprint.pads {
                let Some(drill) = pad.drill else {
                    continue; // SMD pad — no drill, no annular ring check
                };

                // Use the smaller of width/height for worst-case annular ring
                let pad_diameter = pad.size.0.min(pad.size.1);

                // annular_ring = (pad_diameter - drill_diameter) / 2
                // Use saturating sub to avoid underflow on malformed data
                let ring = Nm((pad_diameter.0.saturating_sub(drill.0)) / 2);

                if ring < min_ring {
                    let pad_location = Point::new(
                        Nm(position.0.x.0 + pad.position.x.0),
                        Nm(position.0.y.0 + pad.position.y.0),
                    );
                    violations.push(
                        DrcViolation::annular_ring(entity, ring, min_ring, pad_location)
                            .with_pad_info(refdes.as_str(), &pad.number),
                    );
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ViolationKind;
    use cypcb_world::components::{NetConnections, Rotation, Value};

    #[test]
    fn test_rule_name() {
        assert_eq!(AnnularRingRule.name(), "annular-ring");
    }

    #[test]
    fn test_adequate_annular_ring_passes() {
        // DIP-8: pad diameter = 1.6mm, drill = 1.0mm
        // annular ring = (1.6 - 1.0) / 2 = 0.3mm — passes 0.15mm requirement
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("U1"),
            Value::new("IC"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("DIP-8"),
            NetConnections::new(),
        );

        let rules = DesignRules::jlcpcb_2layer(); // 0.15mm min annular ring
        let violations = AnnularRingRule.check(&mut world, &rules);
        assert!(
            violations.is_empty(),
            "DIP-8 should pass standard annular ring check"
        );
    }

    #[test]
    fn test_insufficient_annular_ring_fails() {
        // DIP-8: pad = 1.6mm, drill = 0.8mm, ring = (1.6 - 0.8) / 2 = 0.4mm
        // Set requirement to 0.5mm — should fail
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("U1"),
            Value::new("IC"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("DIP-8"),
            NetConnections::new(),
        );

        let rules = DesignRules {
            min_annular_ring: Nm::from_mm(0.5), // Larger than DIP-8's 0.4mm ring
            ..DesignRules::default()
        };

        let violations = AnnularRingRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 8, "All 8 DIP-8 pads should fail");
        for v in &violations {
            assert_eq!(v.kind, ViolationKind::AnnularRing);
        }
    }

    #[test]
    fn test_smd_pad_exempt() {
        // 0402 is SMD only — no drill, no annular ring check
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
            min_annular_ring: Nm::from_mm(10.0), // Impossibly large
            ..DesignRules::default()
        };

        let violations = AnnularRingRule.check(&mut world, &rules);
        assert!(
            violations.is_empty(),
            "SMD pads should not trigger annular ring violations"
        );
    }

    #[test]
    fn test_empty_world() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();
        let violations = AnnularRingRule.check(&mut world, &rules);
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
        let violations = AnnularRingRule.check(&mut world, &rules);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_violation_message_includes_dimensions() {
        // DIP-8: pad = 1.6mm, drill = 0.8mm, ring = 0.4mm
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("U1"),
            Value::new("IC"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("DIP-8"),
            NetConnections::new(),
        );

        let rules = DesignRules {
            min_annular_ring: Nm::from_mm(0.5),
            ..DesignRules::default()
        };

        let violations = AnnularRingRule.check(&mut world, &rules);
        assert!(!violations.is_empty());
        // Message should contain actual and required dimensions
        assert!(
            violations[0].message.contains("0.400"),
            "Should show actual ring width (0.4mm)"
        );
        assert!(
            violations[0].message.contains("0.500"),
            "Should show required ring width (0.5mm)"
        );
    }
}
