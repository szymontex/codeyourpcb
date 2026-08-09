//! Trace width against the current a net has to carry.
//!
//! `MinTraceWidthRule` asks whether a trace is wide enough for the fabricator.
//! This asks whether it is wide enough for the design: a net declared as
//! `net VBUS { current 2A }` needs copper in proportion, and a trace that
//! satisfies the fab's minimum can still cook.
//!
//! The width comes from `cypcb-calc`, which is the workspace's one
//! implementation of IPC-2221. Outer layers dissipate heat into the air and
//! inner layers do not, so which layer a trace is on changes the answer.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Trace;
use cypcb_world::components::NetId;
use cypcb_world::{BoardWorld, Layer};

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking trace width against a net's declared current.
pub struct TraceCurrentRule;

impl DrcRule for TraceCurrentRule {
    fn name(&self) -> &'static str {
        "trace-current"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        // Collect first: the constraint lookup borrows the world immutably and
        // the query holds it mutably.
        let traces: Vec<(bevy_ecs::entity::Entity, NetId, Layer, Nm, Point)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Trace)>();
            query
                .iter(ecs)
                .filter_map(|(entity, trace)| {
                    Some((
                        entity,
                        trace.net_id,
                        trace.layer,
                        trace.width,
                        midpoint(trace)?,
                    ))
                })
                .collect()
        };

        let mut violations = Vec::new();
        for (entity, net_id, layer, width, at) in traces {
            let Some(constraints) = world.net_constraints(net_id) else {
                continue;
            };
            let Some(current_ma) = constraints.current_ma else {
                continue;
            };
            if current_ma <= 0.0 {
                continue;
            }

            // The fab's copper, not the calculator's default. IPC-2221 needs
            // the thickness to answer at all, and a number the checker prints
            // should be traceable to the table it came from - every preset
            // says 1.0oz today, which is what the default was, so this moves
            // no number and makes the one it prints explainable.
            let copper_oz = rules.copper_weight_oz_x10 as f64 / 10.0;
            let mut params =
                cypcb_calc::TraceWidthParams::new(current_ma / 1000.0).with_copper_oz(copper_oz);
            if !is_external(layer) {
                params = params.internal();
            }
            let required = cypcb_calc::TraceWidthCalculator::calculate(&params).width;

            if width.raw() < required.raw() {
                let net_name = world.net_name(net_id).unwrap_or("unnamed").to_string();
                let mut violation = DrcViolation::trace_current(entity, width, required, at);
                // What the number assumes, said out loud. `0.5mm` and
                // `0.5mm at 2oz` are different claims, and a reader deciding
                // whether to widen a trace cannot tell them apart.
                violation.message = format!(
                    "trace '{}' is {:.3}mm wide for {}: IPC-2221 wants {:.3}mm on an {} layer at {:.1}oz copper and a {:.0}C rise",
                    net_name,
                    width.raw() as f64 / 1_000_000.0,
                    format_current(current_ma),
                    required.raw() as f64 / 1_000_000.0,
                    if is_external(layer) { "outer" } else { "inner" },
                    copper_oz,
                    params.temp_rise_c,
                );
                violations.push(violation);
            }
        }

        violations
    }
}

/// Outer layers shed heat into the air; inner ones are buried and need more
/// copper for the same current.
fn is_external(layer: Layer) -> bool {
    matches!(layer, Layer::TopCopper | Layer::BottomCopper)
}

/// A point on the trace to report the violation at.
fn midpoint(trace: &Trace) -> Option<Point> {
    let segment = trace.segments.get(trace.segments.len() / 2)?;
    Some(Point::new(
        Nm((segment.start.x.raw() + segment.end.x.raw()) / 2),
        Nm((segment.start.y.raw() + segment.end.y.raw()) / 2),
    ))
}

/// Milliamps below an amp, amps above, the way a schematic states it.
fn format_current(current_ma: f64) -> String {
    if current_ma >= 1000.0 {
        format!("{:.1}A", current_ma / 1000.0)
    } else {
        format!("{:.0}mA", current_ma)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::components::trace::{TraceSegment, TraceSource};
    use cypcb_world::registry::NetConstraints;

    /// A board with one trace of `width_mm` on `layer`, carrying `current_ma`.
    fn board(width_mm: f64, layer: Layer, current_ma: Option<f64>) -> BoardWorld {
        let mut world = BoardWorld::new();
        world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 4);
        let net = world.intern_net("VBUS");
        if let Some(current_ma) = current_ma {
            world.set_net_constraints(
                net,
                NetConstraints {
                    current_ma: Some(current_ma),
                    ..NetConstraints::default()
                },
            );
        }

        let trace = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            )],
            width: Nm::from_mm(width_mm),
            layer,
            net_id: net,
            locked: false,
            source: TraceSource::Manual,
        };
        world.spawn_entity((trace, net));
        world
    }

    #[test]
    fn a_thin_trace_on_a_high_current_net_is_flagged() {
        // 1A external wants about 0.30mm. A default 0.2mm track does not.
        let mut world = board(0.2, Layer::TopCopper, Some(1000.0));
        let violations = TraceCurrentRule.check(&mut world, &DesignRules::jlcpcb_2layer());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, crate::ViolationKind::TraceCurrent);
        assert!(
            violations[0].message.contains("1.0A") && violations[0].message.contains("outer"),
            "got {}",
            violations[0].message
        );
    }

    #[test]
    fn a_wide_enough_trace_passes() {
        let mut world = board(1.0, Layer::TopCopper, Some(1000.0));
        let violations = TraceCurrentRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert!(violations.is_empty(), "got {violations:?}");
    }

    #[test]
    fn a_net_that_declares_no_current_is_not_this_rules_business() {
        let mut world = board(0.05, Layer::TopCopper, None);
        let violations = TraceCurrentRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert!(violations.is_empty(), "got {violations:?}");
    }

    #[test]
    fn an_inner_layer_needs_more_copper_than_an_outer_one() {
        // The same trace, the same current, buried. The rule has to notice.
        let width_mm = 0.4;
        let mut outer = board(width_mm, Layer::TopCopper, Some(1000.0));
        let mut inner = board(width_mm, Layer::Inner(1), Some(1000.0));

        let outer_violations = TraceCurrentRule.check(&mut outer, &DesignRules::jlcpcb_2layer());
        let inner_violations = TraceCurrentRule.check(&mut inner, &DesignRules::jlcpcb_2layer());

        assert!(
            outer_violations.is_empty(),
            "0.4mm carries 1A on an outer layer: {outer_violations:?}"
        );
        assert_eq!(
            inner_violations.len(),
            1,
            "the same width buried does not: {inner_violations:?}"
        );
        assert!(inner_violations[0].message.contains("inner"));
    }
}
