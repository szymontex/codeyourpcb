//! A stated neck, checked rather than trusted.
//!
//! `neck 0.8mm for 4mm` says the copper may run thin on the way into a pad and
//! how far it may do so. Stating it is what separates a deliberate neck from a
//! mistake, but a claim nobody measures is no better than no claim - so this
//! measures the three ways the statement itself can be wrong.
//!
//! Since 2026-08-21 a segment can carry its own width, so there is a fourth
//! thing to measure: **how far the copper actually runs thin in one stretch**,
//! against how far the declaration allows. One stretch, not the sum: the
//! grammar calls a neck "how narrow the copper may get on the way into a pad",
//! and a net whose copper reaches two pads necks twice while obeying the
//! declaration both times. A trace whose segments say nothing is still
//! only checked for a well-formed declaration - there is no thin copper to
//! measure - and the rule says so rather than passing it quietly.
//!
//! What it still does **not** do is decide whether the neck is thermally safe.
//! That needs a current and a temperature rise this model does not carry.

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
        let necked: Vec<Measured> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Trace, &TraceNeck)>();
            query
                .iter(ecs)
                .filter_map(|(entity, trace, neck)| {
                    Some(Measured {
                        entity,
                        neck_width: neck.width,
                        neck_length: neck.length,
                        trace_width: trace.width,
                        trace_length: trace.total_length(),
                        run_thin: trace.longest_necked_stretch(),
                        at: midpoint(trace)?,
                    })
                })
                .collect()
        };

        let mut violations = Vec::new();
        for Measured {
            entity,
            neck_width,
            neck_length,
            trace_width,
            trace_length,
            run_thin,
            at,
        } in necked
        {
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

            // The copper against the claim. `run_thin` is the longest unbroken
            // stretch of copper narrower than the trace - one approach to one
            // pad, which is what `neck <width> for <length>` is a statement
            // about. It was the *sum* until 2026-08-22, which read a net that
            // necks into two pads as one that overran by double.
            if run_thin > neck_length {
                let mut violation = DrcViolation::neck_down(entity, at);
                violation.message = format!(
                    "the copper runs thin for {} in one stretch where the neck allows {}: the declaration is not what was drawn",
                    mm(run_thin),
                    mm(neck_length)
                );
                violations.push(violation);
            }
        }

        violations
    }
}

/// One declared neck and the trace under it, read out of the world in one pass.
///
/// A named struct rather than a tuple because the seventh field was the one
/// that made a reader count commas to find out which `Nm` was which.
struct Measured {
    entity: bevy_ecs::entity::Entity,
    neck_width: Nm,
    neck_length: Nm,
    trace_width: Nm,
    trace_length: Nm,
    /// How far the trace's own segments run narrower than the trace.
    run_thin: Nm,
    at: Point,
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
