//! Copper crossing a fold has to cross it, not follow it.
//!
//! IPC-2223's rule for a bend area is one sentence: route the circuitry
//! **perpendicular to the bend** across the flex area, so every conductor
//! takes the same strain over the fold. A trace that runs along the fold
//! instead takes that strain along its own length, concentrated where it
//! enters and leaves the bend, and cracks there.
//!
//! The fold direction is read from the region rather than stated: a ribbon is
//! a band across the board, and the line it folds about runs the way the band
//! spans it. A `flex` region reaching both left and right edges folds about a
//! line running across the board, so copper should run up and down it; a band
//! reaching top and bottom is the other way round. A region that reaches
//! neither pair of edges, or both, is a shape this rule cannot read a fold
//! direction out of, and it says nothing about one.
//!
//! **45 degrees is the line, and it is geometry rather than a fab figure.** No
//! house publishes an angle: what they publish is "perpendicular". Past 45
//! degrees a segment is running more along the fold than across it, which is
//! the point where the sentence stops being nearly kept and starts being
//! broken. The message carries the measured angle so a designer can see how
//! far off it is.

use cypcb_core::Point;
use cypcb_world::components::trace::Trace;
use cypcb_world::components::{BoardSize, Zone};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for copper that follows a fold instead of crossing it.
pub struct FlexTraceAngleRule;

/// Which way the board folds here, as the axis copper should run along.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Across {
    /// The band spans the board top to bottom, so copper runs left to right.
    X,
    /// The band spans it left to right, so copper runs up and down.
    Y,
}

impl DrcRule for FlexTraceAngleRule {
    fn name(&self) -> &'static str {
        "flex-trace-angle"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let Some(board_entity) = world.board_entity() else {
            return Vec::new();
        };
        let Some(size) = world.ecs().get::<BoardSize>(board_entity).copied() else {
            return Vec::new();
        };

        let folds: Vec<(Zone, Across)> = world
            .zones()
            .into_iter()
            .filter(|(_, zone)| zone.is_flex())
            .filter_map(|(_, zone)| {
                let spans_y = zone.bounds.min.y.0 <= 0 && zone.bounds.max.y.0 >= size.height.0;
                let spans_x = zone.bounds.min.x.0 <= 0 && zone.bounds.max.x.0 >= size.width.0;
                match (spans_x, spans_y) {
                    // A band top to bottom: the fold line runs that way and
                    // copper crosses it left to right.
                    (false, true) => Some((zone, Across::X)),
                    (true, false) => Some((zone, Across::Y)),
                    // A patch in the middle of the board, or the whole board:
                    // neither says which way it folds.
                    _ => None,
                }
            })
            .collect();
        if folds.is_empty() {
            return Vec::new();
        }

        let traces: Vec<(bevy_ecs::entity::Entity, Trace)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Trace)>();
            query
                .iter(ecs)
                .map(|(entity, trace)| (entity, trace.clone()))
                .collect()
        };

        let mut violations = Vec::new();
        for (entity, trace) in traces {
            for segment in &trace.segments {
                let middle = Point::new(
                    cypcb_core::Nm((segment.start.x.0 + segment.end.x.0) / 2),
                    cypcb_core::Nm((segment.start.y.0 + segment.end.y.0) / 2),
                );
                let Some((zone, across)) = folds
                    .iter()
                    .find(|(zone, _): &&(Zone, Across)| zone.contains(middle))
                else {
                    continue;
                };

                let run = (segment.end.x.0 - segment.start.x.0) as f64;
                let rise = (segment.end.y.0 - segment.start.y.0) as f64;
                if run == 0.0 && rise == 0.0 {
                    continue;
                }
                // How far off the direction the fold wants, in degrees, always
                // between 0 and 90: a segment drawn right to left crosses the
                // fold exactly as well as one drawn left to right.
                let (along, sideways) = match *across {
                    Across::X => (run.abs(), rise.abs()),
                    Across::Y => (rise.abs(), run.abs()),
                };
                let off = sideways.atan2(along).to_degrees();
                if off <= 45.0 {
                    continue;
                }

                let where_it_is = match &zone.name {
                    Some(name) => format!("the flexible region '{name}'"),
                    None => "a flexible region".to_string(),
                };
                violations.push(DrcViolation::flex_trace_angle(
                    entity,
                    format!(
                        "a trace in {where_it_is} runs {off:.0} degrees off the fold's own \
                         direction, which is more along the bend than across it: copper over a \
                         fold takes the strain along its length and cracks where it enters and \
                         leaves, so IPC-2223 routes a bend area perpendicular to the bend"
                    ),
                    middle,
                ));
                // One report per trace: a run of segments following the fold
                // is one mistake, and a row per chord would bury the panel.
                break;
            }
        }
        violations
    }
}
