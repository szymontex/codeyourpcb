//! Hole-to-hole clearance rule.
//!
//! Checks minimum distance between drill holes (edge-to-edge).
//! Applies to through-hole pads, vias, and mounting holes.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Via;
use cypcb_world::components::{FootprintRef, Position, Rotation};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::{rotate_point, DrcRule};

/// Rule for checking minimum hole-to-hole clearance.
pub struct HoleToHoleRule;

impl DrcRule for HoleToHoleRule {
    fn name(&self) -> &'static str {
        "hole-to-hole"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_distance = rules.min_hole_to_hole;

        // Every drilled feature on the board: vias and through-hole pads. A via
        // 0.2mm from a connector pin is as unmanufacturable as two vias that
        // close, so checking only via-to-via missed most of the real cases.
        let mut holes: Vec<(bevy_ecs::entity::Entity, Point, Nm)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Via)>();
            query
                .iter(ecs)
                .map(|(e, v)| (e, v.position, v.drill))
                .collect()
        };

        let components: Vec<_> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(
                bevy_ecs::entity::Entity,
                &FootprintRef,
                &Position,
                &Rotation,
            )>();
            query
                .iter(ecs)
                .map(|(e, f, p, r)| (e, f.clone(), *p, *r))
                .collect()
        };

        // The board carries the table it was synced with, including any footprint
        // the source defined inline; building a fresh one here would see built-ins only.
        let lib = world.footprints();
        for (entity, footprint_ref, position, rotation) in components {
            let Some(footprint) = lib.get(footprint_ref.as_str()) else {
                continue; // Unknown footprint - sync already reported it
            };
            for pad in &footprint.pads {
                let Some(drill) = pad.drill else { continue };
                let offset = rotate_point(pad.position, rotation.to_degrees());
                holes.push((
                    entity,
                    Point::new(
                        Nm(position.0.x.0 + offset.x.0),
                        Nm(position.0.y.0 + offset.y.0),
                    ),
                    drill,
                ));
            }
        }

        // Check all pairs
        for i in 0..holes.len() {
            for j in (i + 1)..holes.len() {
                let (e_a, pos_a, drill_a) = &holes[i];
                let (e_b, pos_b, drill_b) = &holes[j];

                // Two pads of the same component are placed by the footprint,
                // not by the designer, so a footprint's own pitch is not a
                // board defect to report here.
                if e_a == e_b {
                    continue;
                }

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
