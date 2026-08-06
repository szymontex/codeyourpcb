//! Courtyard clearance rule.
//!
//! Checks that component courtyards don't overlap or are too close.
//! This prevents physical interference during assembly.

use cypcb_core::{Nm, Point};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking minimum courtyard clearance between components.
pub struct CourtyardClearanceRule;

impl DrcRule for CourtyardClearanceRule {
    fn name(&self) -> &'static str {
        "courtyard-clearance"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_clearance = rules.min_courtyard_clearance;

        // Take the courtyards from the components themselves.
        //
        // This used to filter the spatial index for entries with
        // `layer_mask == 0`, on the understanding that those were the courtyard
        // entries. No index builder in the crate sets that: components are
        // indexed with `0xFFFFFFFF`, so the filter matched nothing and the rule
        // reported nothing on every real board. A rule whose input is empty by
        // construction is worse than no rule, because the report says zero.
        let courtyards = component_courtyards(world);

        // Check all pairs of courtyard AABBs
        for i in 0..courtyards.len() {
            for j in (i + 1)..courtyards.len() {
                let a = &courtyards[i];
                let b = &courtyards[j];

                // Skip if same entity
                if a.entity == b.entity {
                    continue;
                }

                // AABB gap in each dimension
                let dx = (a.min[0].max(b.min[0]) - a.max[0].min(b.max[0])).max(0);
                let dy = (a.min[1].max(b.min[1]) - a.max[1].min(b.max[1])).max(0);

                let distance = if dx == 0 && dy == 0 {
                    0 // Overlapping
                } else {
                    let dx_sq = (dx as i128) * (dx as i128);
                    let dy_sq = (dy as i128) * (dy as i128);
                    ((dx_sq + dy_sq) as f64).sqrt() as i64
                };

                if distance < min_clearance.0 {
                    let location =
                        Point::new(Nm((a.min[0] + a.max[0]) / 2), Nm((a.min[1] + a.max[1]) / 2));
                    violations.push(DrcViolation::courtyard_clearance(
                        a.entity,
                        b.entity,
                        Nm(distance),
                        min_clearance,
                        location,
                    ));
                }
            }
        }

        violations
    }
}

/// One component's courtyard, in board coordinates.
struct Courtyard {
    entity: bevy_ecs::entity::Entity,
    min: [i64; 2],
    max: [i64; 2],
}

/// Every placed component's courtyard, taken from the footprint library.
///
/// A part whose footprint is not in the library is skipped: without the
/// footprint there is no courtyard to compare, and inventing one would report
/// collisions that are an artefact of the guess.
fn component_courtyards(world: &mut BoardWorld) -> Vec<Courtyard> {
    use cypcb_world::components::{FootprintRef, Position, Rotation};

    let placements: Vec<(bevy_ecs::entity::Entity, cypcb_core::Point, f64, String)> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(
            bevy_ecs::entity::Entity,
            &Position,
            &Rotation,
            &FootprintRef,
        )>();
        query
            .iter(ecs)
            .map(|(entity, position, rotation, footprint)| {
                (
                    entity,
                    position.0,
                    rotation.to_degrees(),
                    footprint.as_str().to_string(),
                )
            })
            .collect()
    };

    let library = world.footprints();

    placements
        .into_iter()
        .filter_map(|(entity, position, degrees, name)| {
            let courtyard = library.get(&name)?.courtyard;

            // A rotated part occupies the extent of its rotated courtyard.
            // Boxing that extent can only make the keepout larger, which is the
            // safe direction for a placement rule.
            let radians = degrees.to_radians();
            let (sin, cos) = radians.sin_cos();
            let half_w = (courtyard.max.x.0 - courtyard.min.x.0) as f64 / 2.0;
            let half_h = (courtyard.max.y.0 - courtyard.min.y.0) as f64 / 2.0;
            let extent_x = (half_w * cos.abs() + half_h * sin.abs()).round() as i64;
            let extent_y = (half_w * sin.abs() + half_h * cos.abs()).round() as i64;

            let local_cx = (courtyard.min.x.0 + courtyard.max.x.0) / 2;
            let local_cy = (courtyard.min.y.0 + courtyard.max.y.0) / 2;
            let cx = position.x.0 + (local_cx as f64 * cos - local_cy as f64 * sin).round() as i64;
            let cy = position.y.0 + (local_cx as f64 * sin + local_cy as f64 * cos).round() as i64;

            Some(Courtyard {
                entity,
                min: [cx - extent_x, cy - extent_y],
                max: [cx + extent_x, cy + extent_y],
            })
        })
        .collect()
}
