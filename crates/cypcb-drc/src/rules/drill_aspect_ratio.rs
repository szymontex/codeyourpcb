//! A hole can be too deep for its own width to be plated.
//!
//! Plating a through hole is chemistry, not machining: copper is pulled down
//! the barrel out of solution. Past some depth-to-width ratio the solution no
//! longer refreshes in the middle of the hole, and the board comes back with a
//! barrel that is thin or open somewhere a person cannot see. Every fab
//! publishes the ratio it will still plate - 8:1 on JLCPCB's standard process,
//! 12:1 on its advanced one - and nothing in this workspace read the number.
//!
//! It could not, until now. Aspect ratio is thickness divided by drill, and
//! the checker had no thickness: `stackup` was parsed and dropped on the floor.
//! Now that a declared stackup reaches the model, the depth of every hole on
//! the board is a number this rule can divide by, and a design that says
//! nothing takes the fab's own standard thickness rather than a constant.
//!
//! Only plated holes are asked. A mounting hole is drilled and left bare, so
//! there is no plating in it to fail - `PadDef::is_non_plated` is the same
//! question the drill file asks when it decides which file a hole belongs in.
//!
//! `max_drill_aspect_ratio` and `board_thickness` are two of the fifteen
//! numbers every fab preset published with nothing in the workspace reading
//! them. This closes both.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Via;
use cypcb_world::components::{FootprintRef, Position, Rotation};
use cypcb_world::BoardWorld;

use super::{rotate_point, DrcRule};
use crate::presets::DesignRules;
use crate::violation::{smallest_platable_drill, DrcViolation};

/// Rule for checking how deep each plated hole is for its width.
pub struct DrillAspectRatioRule;

impl DrcRule for DrillAspectRatioRule {
    fn name(&self) -> &'static str {
        "drill-aspect-ratio"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        // The design's own stackup wins, because a board that states how it is
        // built is stating how deep its holes are. A design that says nothing
        // is built at the fab's standard thickness.
        let thickness = world
            .stackup()
            .and_then(|stackup| stackup.total_thickness())
            .unwrap_or(rules.board_thickness);

        let smallest = smallest_platable_drill(thickness, rules.max_drill_aspect_ratio);
        if smallest <= Nm(0) {
            return Vec::new(); // No published ratio, nothing to grade against.
        }

        let mut violations = Vec::new();

        // Vias. Every via is plated - a via that is not plated joins nothing.
        {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Via)>();
            for (entity, via) in query.iter(ecs) {
                if via.drill < smallest {
                    violations.push(DrcViolation::drill_aspect_ratio(
                        entity,
                        via.drill,
                        thickness,
                        rules.max_drill_aspect_ratio,
                        via.position,
                    ));
                }
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
                if pad.is_non_plated() {
                    continue; // A bare hole has no plating to fail.
                }
                if drill >= smallest {
                    continue;
                }
                let offset = rotate_point(pad.position, degrees);
                violations.push(DrcViolation::drill_aspect_ratio(
                    *entity,
                    drill,
                    thickness,
                    rules.max_drill_aspect_ratio,
                    Point::new(
                        Nm(position.0.x.0 + offset.x.0),
                        Nm(position.0.y.0 + offset.y.0),
                    ),
                ));
            }
        }

        violations
    }
}
