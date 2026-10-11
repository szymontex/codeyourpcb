//! Annular ring rule.
//!
//! Validates that through-hole pads have sufficient copper ring width
//! around the drill hole. The annular ring is:
//! `(pad_diameter - drill_diameter) / 2`.
//!
//! For non-circular pads, the smaller of width/height is used as the
//! effective pad diameter (worst-case check).
//!
//! A pad whose hole is not in the middle of its copper
//! ([`PadDef::hole_offset`]) is measured where the hole is: the ring is the
//! smallest gap between the hole's edge and the pad's edge, which is narrower
//! on the side the hole is moved towards.

use cypcb_core::{Nm, Point};
use cypcb_world::components::{FootprintRef, PadShape, Position, RefDes};
use cypcb_world::footprint::PadDef;
use cypcb_world::in_build_order;
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
            in_build_order::<(bevy_ecs::entity::Entity, &RefDes, &FootprintRef, &Position)>(ecs)
                .into_iter()
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
                // An annular ring is the copper around a hole, and a mounting
                // hole has none by definition - measuring one reported every
                // hole as 0.000mm against a 0.150mm minimum.
                if pad.is_non_plated() {
                    continue;
                }
                let Some(drill) = pad.drill else {
                    continue; // SMD pad — no drill, no annular ring check
                };

                let ring = if pad.hole_offset.is_some() {
                    ring_round_an_offset_hole(pad, drill)
                } else {
                    // Use the smaller of width/height for worst-case annular ring
                    let pad_diameter = pad.size.0.min(pad.size.1);

                    // annular_ring = (pad_diameter - drill_diameter) / 2
                    // Use saturating sub to avoid underflow on malformed data
                    Nm((pad_diameter.0.saturating_sub(drill.0)) / 2)
                };

                if ring < min_ring {
                    let hole = pad.hole_centre();
                    let pad_location =
                        Point::new(Nm(position.0.x.0 + hole.x.0), Nm(position.0.y.0 + hole.y.0));
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

/// The ring round a hole that is not in the middle of its pad: the shortest
/// way from the hole's edge to the pad's edge, in the pad's own frame.
///
/// A hole is the path of its bit's centre - one point for a drilled hole, two
/// ends for a slot - grown by the bit's radius. The distance from a point
/// inside a convex pad to the pad's edge is a concave function of the point, so
/// the smallest over the path is at one of its ends, and the ring is that
/// smallest distance less the radius. The distance itself is that of a
/// rounded rectangle, which every shape this project has is: a rectangle is
/// one with no rounding, a rounded rectangle has its own, and a circle or an
/// oblong is one rounded by half its narrow side.
fn ring_round_an_offset_hole(pad: &PadDef, drill: Nm) -> Nm {
    let (half_w, half_h) = (pad.size.0 .0 as f64 / 2.0, pad.size.1 .0 as f64 / 2.0);
    let rounding = match pad.shape {
        PadShape::Rect => 0.0,
        PadShape::RoundRect { corner_ratio } => {
            pad.size.0 .0.min(pad.size.1 .0) as f64 * f64::from(corner_ratio) / 100.0
        }
        PadShape::Circle | PadShape::Oblong => half_w.min(half_h),
    };
    let offset = pad.hole_offset.unwrap_or(Point::ORIGIN);
    let travel = pad
        .slot_half_travel_in_the_pad_frame()
        .unwrap_or(Point::ORIGIN);
    let from_the_edge = |x: f64, y: f64| {
        let (qx, qy) = (
            (x.abs() - (half_w - rounding)).max(0.0),
            (y.abs() - (half_h - rounding)).max(0.0),
        );
        if qx == 0.0 && qy == 0.0 {
            (half_w - x.abs()).min(half_h - y.abs())
        } else {
            rounding - (qx * qx + qy * qy).sqrt()
        }
    };
    let (ox, oy) = (offset.x.0 as f64, offset.y.0 as f64);
    let (tx, ty) = (travel.x.0 as f64, travel.y.0 as f64);
    let nearest = from_the_edge(ox - tx, oy - ty).min(from_the_edge(ox + tx, oy + ty));
    Nm((nearest - drill.0 as f64 / 2.0).round().max(0.0) as i64)
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
