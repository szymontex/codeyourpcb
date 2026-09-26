//! Via diameter rule.
//!
//! Checks that via outer diameters meet minimum requirements.

use cypcb_world::components::trace::Via;
use cypcb_world::in_build_order;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking minimum via outer diameter.
pub struct ViaDiameterRule;

impl DrcRule for ViaDiameterRule {
    fn name(&self) -> &'static str {
        "via-diameter"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_diameter = rules.min_via_diameter;

        let ecs = world.ecs_mut();
        let vias: Vec<_> = in_build_order::<(bevy_ecs::entity::Entity, &Via)>(ecs)
            .into_iter()
            .map(|(e, v)| (e, v.position, v.outer_diameter))
            .collect();

        for (entity, position, diameter) in vias {
            if diameter < min_diameter {
                violations.push(DrcViolation::via_diameter(
                    entity,
                    diameter,
                    min_diameter,
                    position,
                ));
            }
        }

        violations
    }
}
