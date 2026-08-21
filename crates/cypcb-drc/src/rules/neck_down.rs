//! A stated neck, checked rather than trusted.
//!
//! `neck 0.8mm for 4mm` says the copper may run thin on the way into a pad and
//! how far it may do so. Stating it is what separates a deliberate neck from a
//! mistake, but a claim nobody measures is no better than no claim - so this
//! measures the three ways the statement itself can be wrong.
//!
//! What it does **not** do is decide whether the neck is thermally safe. That
//! needs the copper's real geometry, and a trace in this model carries one
//! width; the necked stretch is not in the segments yet. Until it is, the
//! honest position is that the declaration is well formed, not that the board
//! is.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::{Trace, TraceNeck};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking a declared neck against the trace and the fabricator.
pub struct NeckDownRule;

impl DrcRule for NeckDownRule {
    fn name(&self) -> &'static str {
        "neck-down"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let necked: Vec<(bevy_ecs::entity::Entity, Nm, Nm, Nm, Nm, Point)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Trace, &TraceNeck)>();
            query
                .iter(ecs)
                .filter_map(|(entity, trace, neck)| {
                    Some((
                        entity,
                        neck.width,
                        neck.length,
                        trace.width,
                        trace.total_length(),
                        midpoint(trace)?,
                    ))
                })
                .collect()
        };

        let mut violations = Vec::new();
        for (entity, neck_width, neck_length, trace_width, trace_length, at) in necked {
            if neck_width >= trace_width {
                let mut violation = DrcViolation::neck_down(entity, at);
                violation.message = format!(
                    "a neck of {} is not narrower than the {} trace it is on: a neck that does not narrow is a second width",
                    mm(neck_width),
                    mm(trace_width)
                );
                violations.push(violation);
            } else if neck_width < rules.min_trace_width {
                // Only when it really is a neck. A width that failed the test
                // above is reported once, for the reason that fits it.
                let mut violation = DrcViolation::neck_down(entity, at);
                violation.message = format!(
                    "a neck of {} is under the {} this fabricator will etch: the board cannot be made whatever the neck is for",
                    mm(neck_width),
                    mm(rules.min_trace_width)
                );
                violations.push(violation);
            }

            if neck_length > trace_length {
                let mut violation = DrcViolation::neck_down(entity, at);
                violation.message = format!(
                    "a neck allowed to run {} is on a trace {} long: the whole trace is the neck, so nothing is carrying the current",
                    mm(neck_length),
                    mm(trace_length)
                );
                violations.push(violation);
            }
        }

        violations
    }
}

/// A point on the trace to report the violation at.
fn midpoint(trace: &Trace) -> Option<Point> {
    let segment = trace.segments.get(trace.segments.len() / 2)?;
    Some(Point::new(
        Nm((segment.start.x.raw() + segment.end.x.raw()) / 2),
        Nm((segment.start.y.raw() + segment.end.y.raw()) / 2),
    ))
}

/// Millimetres, the way the language writes one.
fn mm(value: Nm) -> String {
    format!("{}mm", value.raw() as f64 / 1_000_000.0)
}
