//! A hole too near the edge is a hole the router breaks into.
//!
//! The board is cut out of a larger panel by a milling bit that follows the
//! outline. A drilled hole whose wall sits closer to that path than the fab
//! allows comes out of the machine open on one side: a mounting hole with a
//! notch, or a plated hole whose barrel is gone.
//!
//! Not the same question as [`EdgeClearanceRule`], which measures **copper**
//! against the edge. A pad's annulus is wider than the hole inside it, so a
//! pad can clear the edge by the copper rule while its own drill does not -
//! and it is the hole the bit breaks into, not the annulus.
//!
//! `min_hole_to_edge` is one of fifteen numbers every fab preset published
//! with nothing in the workspace reading them. This is the second one closed.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Via;
use cypcb_world::components::{BoardOutline, BoardSize, FootprintRef, Position, Rotation};
use cypcb_world::BoardWorld;

use super::{rotate_point, DrcRule};
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// One drilled hole, as a circle.
struct Hole {
    entity: bevy_ecs::entity::Entity,
    centre: Point,
    radius: i64,
}

/// Rule for checking drilled holes against the routed board edge.
pub struct HoleToEdgeRule;

impl DrcRule for HoleToEdgeRule {
    fn name(&self) -> &'static str {
        "hole-to-edge"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let min_gap = rules.min_hole_to_edge.0;

        let Some(board_entity) = world.board_entity() else {
            return Vec::new();
        };
        let Some(board_size) = world.ecs().get::<BoardSize>(board_entity).copied() else {
            return Vec::new();
        };
        let outline = world.ecs().get::<BoardOutline>(board_entity).cloned();

        let mut holes: Vec<Hole> = Vec::new();

        // Vias.
        {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Via)>();
            for (entity, via) in query.iter(ecs) {
                holes.push(Hole {
                    entity,
                    centre: via.position,
                    radius: via.drill.0 / 2,
                });
            }
        }

        // Drilled pads, placed the way every other rule places them.
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

        let library = world.footprints();
        for (entity, footprint_ref, position, rotation) in &components {
            let Some(footprint) = library.get(footprint_ref.as_str()) else {
                continue; // Unknown footprint - sync already reported it
            };
            let degrees = rotation.to_degrees();

            for pad in &footprint.pads {
                let Some(drill) = pad.drill else {
                    continue;
                };
                let offset = rotate_point(pad.position, degrees);
                holes.push(Hole {
                    entity: *entity,
                    centre: Point::new(
                        Nm(position.0.x.0 + offset.x.0),
                        Nm(position.0.y.0 + offset.y.0),
                    ),
                    radius: drill.0 / 2,
                });
            }
        }

        let mut violations = Vec::new();
        for hole in &holes {
            // The hole's own bounding box, so the shared outline distance can
            // measure it: for a circle against a straight cut this is the wall
            // of the hole, which is what the bit meets.
            let (min_x, min_y) = (hole.centre.x.0 - hole.radius, hole.centre.y.0 - hole.radius);
            let (max_x, max_y) = (hole.centre.x.0 + hole.radius, hole.centre.y.0 + hole.radius);

            let gap = match &outline {
                Some(outline) => {
                    super::edge_clearance::distance_to_outline(outline, min_x, min_y, max_x, max_y)
                }
                None => {
                    let left = min_x;
                    let bottom = min_y;
                    let right = board_size.width.0 - max_x;
                    let top = board_size.height.0 - max_y;
                    left.min(bottom).min(right).min(top)
                }
            };

            if gap < min_gap {
                violations.push(DrcViolation::hole_to_edge(
                    hole.entity,
                    Nm(gap.max(0)),
                    Nm(min_gap),
                    hole.centre,
                ));
            }
        }

        violations
    }
}
