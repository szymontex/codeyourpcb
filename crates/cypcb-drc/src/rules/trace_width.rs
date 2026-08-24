//! Minimum trace width rule (DRC-02).
//!
//! Every routed or hand-drawn trace has to be at least as wide as the fab can
//! etch, and at least as wide as its own net asked for. The width lives on the
//! `Trace` component and the statement on the net, so one pass over the trace
//! entities is enough - no geometry work.
//!
//! # Design Rules Reference
//!
//! `DesignRules.min_trace_width` comes from the manufacturer's constraints, the
//! same table the autorouter routes against:
//! - JLCPCB 2-layer: 0.127mm (5 mil)
//! - JLCPCB 4-layer: 0.10mm (4 mil)
//! - PCBWay standard: 0.15mm
//! - Prototype: 0.25mm (10 mil)

use std::collections::HashMap;

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Trace;
use cypcb_world::components::NetId;
use cypcb_world::BoardWorld;

use super::DrcRule;
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// Rule that checks all traces meet minimum width.
///
/// # Examples
///
/// ```rust,ignore
/// use cypcb_drc::rules::{MinTraceWidthRule, DrcRule};
/// use cypcb_drc::presets::DesignRules;
///
/// let rule = MinTraceWidthRule;
/// let mut world = BoardWorld::new();
/// // ... spawn traces ...
/// let rules = DesignRules::jlcpcb_2layer(); // min_trace_width = 0.127mm
/// let violations = rule.check(&mut world, &rules);
/// ```
pub struct MinTraceWidthRule;

/// Midpoint of a trace, used to place the violation marker.
///
/// Falls back to the origin for a trace with no segments - which the sync layer
/// does not produce, but the ECS does not forbid either.
fn trace_midpoint(trace: &Trace) -> Point {
    let Some(first) = trace.segments.first() else {
        return Point::ORIGIN;
    };
    let last = trace.segments.last().unwrap_or(first);
    Point::new(
        Nm((first.start.x.0 + last.end.x.0) / 2),
        Nm((first.start.y.0 + last.end.y.0) / 2),
    )
}

impl DrcRule for MinTraceWidthRule {
    fn name(&self) -> &'static str {
        "min-trace-width"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let min_width = rules.min_trace_width;

        // What each net's own block asks for. `net POWER [width 0.5mm]` states
        // a rule the fab table knows nothing about, and the router already
        // honours it: `ruleset_for_world` raises `min_trace_width` for that net
        // before a single segment is drawn. A checker reading the table alone
        // passed a 0.2mm trace on a net that asked for 0.5mm - the same board,
        // routed and checked, disagreed with itself about the same statement.
        //
        // The fab floor still wins where it is the wider of the two. A net
        // asking for less than the house can etch is asking for something
        // nobody can make, which is a different fault and belongs to whoever
        // reads the statement rather than to the trace that obeyed it.
        let net_width: HashMap<u32, Nm> = {
            let ids: Vec<u32> = world.nets().map(|(net, _name)| net.id()).collect();
            ids.into_iter()
                .filter_map(|id| {
                    let stated = world.net_constraints(NetId::new(id))?.width?;
                    Some((id, stated))
                })
                .collect()
        };

        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Trace)>();

        query
            .iter(ecs)
            .filter_map(|(entity, trace)| {
                let required = net_width
                    .get(&trace.net_id.id())
                    .copied()
                    .map_or(min_width, |stated| min_width.max(stated));
                (trace.width < required).then(|| {
                    DrcViolation::trace_width(entity, trace.width, required, trace_midpoint(trace))
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::components::trace::{TraceSegment, TraceSource};
    use cypcb_world::components::Layer;
    use cypcb_world::NetId;

    fn spawn_trace(world: &mut BoardWorld, width_mm: f64) -> bevy_ecs::entity::Entity {
        let trace = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(10.0, 10.0),
                Point::from_mm(20.0, 10.0),
            )],
            width: Nm::from_mm(width_mm),
            layer: Layer::TopCopper,
            net_id: NetId::new(1),
            locked: false,
            source: TraceSource::Manual,
        };
        world.spawn_entity(trace)
    }

    #[test]
    fn test_rule_name() {
        assert_eq!(MinTraceWidthRule.name(), "min-trace-width");
    }

    #[test]
    fn no_traces_no_violations() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();
        assert!(MinTraceWidthRule.check(&mut world, &rules).is_empty());
    }

    #[test]
    fn trace_at_the_minimum_passes() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::jlcpcb_2layer();
        spawn_trace(&mut world, rules.min_trace_width.to_mm());
        assert!(MinTraceWidthRule.check(&mut world, &rules).is_empty());
    }

    #[test]
    fn trace_below_the_minimum_is_reported_once() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::jlcpcb_2layer();
        spawn_trace(&mut world, 0.05);
        spawn_trace(&mut world, 0.3);

        let violations = MinTraceWidthRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, crate::ViolationKind::TraceWidth);
        // Marker sits on the trace, not at the origin.
        assert_eq!(violations[0].location, Point::from_mm(15.0, 10.0));
        assert!(violations[0].message.contains("0.050mm actual"));
    }

    #[test]
    fn stricter_preset_catches_more() {
        let mut world = BoardWorld::new();
        spawn_trace(&mut world, 0.2);

        let relaxed = DesignRules::jlcpcb_2layer(); // 0.127mm
        let strict = DesignRules::prototype(); // 0.25mm
        assert!(MinTraceWidthRule.check(&mut world, &relaxed).is_empty());
        assert_eq!(MinTraceWidthRule.check(&mut world, &strict).len(), 1);
    }
}
