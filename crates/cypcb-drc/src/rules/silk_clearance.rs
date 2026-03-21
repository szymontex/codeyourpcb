//! Silkscreen clearance rule (stub).
//!
//! Checks that silkscreen features don't overlap copper pads.
//! Full implementation requires silkscreen geometry data per component,
//! which is not yet modeled in the ECS.

use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking silkscreen to copper clearance.
///
/// Currently a stub — requires silkscreen outline geometry per component
/// to compute silk-to-pad distances.
pub struct SilkClearanceRule;

impl DrcRule for SilkClearanceRule {
    fn name(&self) -> &'static str {
        "silk-clearance"
    }

    fn check(&self, _world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        // TODO: Implement when silkscreen geometry is modeled.
        // Algorithm: for each silkscreen line/arc segment, check distance
        // to all copper pads on the same side. Flag if < min_silk_clearance.
        Vec::new()
    }
}
