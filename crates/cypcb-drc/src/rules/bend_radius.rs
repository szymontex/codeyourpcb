//! A fold tighter than the ribbon takes.
//!
//! A flex board is bent round something, and the copper on the outside of the
//! fold is stretched. How far is set by the radius against the ribbon's own
//! thickness, which is why every house publishes the figure as a multiple
//! rather than as a length: JLCPCB states "Single layer: >= 6x total
//! thickness" and "Multi-layer: >= 10x total thickness" on its flex
//! capabilities page.
//!
//! Both halves of that arithmetic are already here. `flex bend { ... radius
//! 3mm }` states the fold, and `Stackup::thickness_in_area` gives what is
//! pressed over that ribbon - the layers that stop elsewhere are not in it, so
//! a stiffener bonded under the rigid end does not thicken the bend.
//!
//! Silent unless the design and the house both say something: a region with no
//! radius, a stack that states no thickness for it, or a table with no
//! published multiple each mean this rule has nothing to measure. A figure
//! invented here is one a designer gets turned away for.

use cypcb_world::components::{BendRadius, Zone};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for a bend radius the ribbon's own thickness refuses.
pub struct BendRadiusRule;

impl DrcRule for BendRadiusRule {
    fn name(&self) -> &'static str {
        "bend-radius"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let Some(stack) = world.stackup().cloned() else {
            return Vec::new();
        };

        let folds: Vec<(bevy_ecs::entity::Entity, Zone, cypcb_core::Nm)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Zone, &BendRadius)>();
            query
                .iter(ecs)
                .filter(|(_, zone, _)| zone.is_flex())
                .map(|(entity, zone, radius)| (entity, zone.clone(), radius.0))
                .collect()
        };

        let mut violations = Vec::new();
        for (entity, zone, radius) in folds {
            let name = zone.name.clone().unwrap_or_default();
            let Some(thickness) = stack.thickness_in_area(&name) else {
                // A stack with a layer that states no thickness. A partial sum
                // would read like a measurement.
                continue;
            };
            if thickness.0 <= 0 {
                continue;
            }

            // One copper layer bends differently from two, and the house says
            // so with two figures. The count is of the copper that is actually
            // over this ribbon, not of the board's own layer count: a
            // four-layer rigid-flex has two of them through the bend.
            let copper_here = stack
                .layers_in_area(&name)
                .into_iter()
                .filter(|layer| {
                    matches!(
                        layer.kind,
                        cypcb_world::components::StackupLayerKind::Copper
                    )
                })
                .count();
            let multiple = if copper_here <= 1 {
                rules.bend_radius_multiple_single
            } else {
                rules.bend_radius_multiple_multilayer
            };
            let Some(multiple) = multiple else {
                continue;
            };

            let least = cypcb_core::Nm(thickness.0 * i64::from(multiple));
            if radius >= least {
                continue;
            }
            violations.push(DrcViolation::bend_radius(
                entity,
                radius,
                least,
                format!(
                    "the fold at '{name}' is {:.3}mm and this house bends {} copper layer(s) of \
                     {:.3}mm no tighter than {}x that, which is {:.3}mm: the copper on the \
                     outside of a tighter fold is stretched past what the laminate holds",
                    radius.to_mm(),
                    copper_here,
                    thickness.to_mm(),
                    multiple,
                    least.to_mm()
                ),
                zone.bounds.min,
            ));
        }
        violations
    }
}
