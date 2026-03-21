//! Hole-to-hole clearance rule.
//!
//! Checks minimum distance between drill holes (edge-to-edge).
//! Applies to through-hole pads, vias, and mounting holes.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Via;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking minimum hole-to-hole clearance.
pub struct HoleToHoleRule;

impl DrcRule for HoleToHoleRule {
    fn name(&self) -> &'static str {
        "hole-to-hole"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_distance = rules.min_hole_to_hole;

        // Collect all entities with drill holes: vias
        let vias: Vec<_> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Via)>();
            query
                .iter(ecs)
                .map(|(e, v)| (e, v.position, v.drill))
                .collect()
        };

        // TODO: Also collect through-hole pad drills from footprint library
        // For now we only check via-to-via distances.

        // Check all pairs
        for i in 0..vias.len() {
            for j in (i + 1)..vias.len() {
                let (e_a, pos_a, drill_a) = &vias[i];
                let (e_b, pos_b, drill_b) = &vias[j];

                // Center-to-center distance
                let dx = (pos_a.x.0 - pos_b.x.0) as f64;
                let dy = (pos_a.y.0 - pos_b.y.0) as f64;
                let center_dist = (dx * dx + dy * dy).sqrt() as i64;

                // Edge-to-edge = center - radius_a - radius_b
                let edge_dist = center_dist - drill_a.0 / 2 - drill_b.0 / 2;

                if edge_dist < min_distance.0 {
                    let location = Point::new(
                        Nm((pos_a.x.0 + pos_b.x.0) / 2),
                        Nm((pos_a.y.0 + pos_b.y.0) / 2),
                    );
                    violations.push(DrcViolation::hole_to_hole(
                        *e_a,
                        *e_b,
                        Nm(edge_dist.max(0)),
                        min_distance,
                        location,
                    ));
                }
            }
        }

        violations
    }
}
