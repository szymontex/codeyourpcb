//! Minimum trace width rule (DRC-02).
//!
//! NOTE: This rule is a placeholder. Trace entities do not exist in the current
//! board model (Phase 1-3). When traces are added in Phase 5 (autorouting),
//! this rule will be implemented to check trace widths against min_trace_width.
//!
//! # Future Implementation
//!
//! When Phase 5 adds the Trace ECS component, this rule will:
//! 1. Query all Trace entities from the board world
//! 2. Check each trace's width against `rules.min_trace_width`
//! 3. Report violations with exact location on the trace segment
//!
//! # Design Rules Reference
//!
//! The `DesignRules.min_trace_width` field is already defined:
//! - JLCPCB 2-layer: 0.15mm (6 mil)
//! - JLCPCB 4-layer: 0.10mm (4 mil)
//! - PCBWay standard: 0.15mm
//! - Prototype: 0.25mm (10 mil)

use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;
use super::DrcRule;

/// Rule that checks all traces meet minimum width.
///
/// Currently a no-op placeholder. Will be implemented when trace entities
/// are added to the board model (Phase 5).
///
/// # Examples
///
/// ```rust,ignore
/// use cypcb_drc::rules::{MinTraceWidthRule, DrcRule};
/// use cypcb_drc::presets::DesignRules;
///
/// let rule = MinTraceWidthRule;
/// let mut world = BoardWorld::new();
/// // ... traces will exist in Phase 5 ...
/// let rules = DesignRules::jlcpcb_2layer(); // min_trace = 0.15mm
/// let violations = rule.check(&mut world, &rules);
/// ```
pub struct MinTraceWidthRule;

impl DrcRule for MinTraceWidthRule {
    fn name(&self) -> &'static str {
        "min-trace-width"
    }

    fn check(&self, _world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        // TODO: Implement when Trace component exists (Phase 5)
        //
        // Future implementation pseudocode:
        // ```
        // let min_width = rules.min_trace_width;
        // let ecs = world.ecs_mut();
        // let mut query = ecs.query::<(Entity, &Trace, &Layer)>();
        //
        // for (entity, trace, layer) in query.iter(ecs) {
        //     if trace.width < min_width {
        //         violations.push(DrcViolation::trace_width(
        //             entity,
        //             trace.width,
        //             min_width,
        //             trace.center_point(),
        //         ));
        //     }
        // }
        // ```
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_name() {
        assert_eq!(MinTraceWidthRule.name(), "min-trace-width");
    }

    #[test]
    fn test_trace_width_rule_is_placeholder() {
        // Placeholder returns no violations (no traces exist yet)
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();
        let violations = MinTraceWidthRule.check(&mut world, &rules);
        assert!(violations.is_empty(), "Placeholder should return no violations");
    }

    #[test]
    fn test_placeholder_with_different_presets() {
        // Verify placeholder works with all presets
        let mut world = BoardWorld::new();

        for preset in crate::Preset::all() {
            let rules = preset.rules();
            let violations = MinTraceWidthRule.check(&mut world, &rules);
            assert!(
                violations.is_empty(),
                "Placeholder should return empty for {:?} preset",
                preset
            );
        }
    }
}
