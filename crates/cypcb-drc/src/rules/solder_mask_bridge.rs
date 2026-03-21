//! Solder mask bridge rule (stub).
//!
//! Checks minimum solder mask web between adjacent pads.
//! Full implementation requires solder mask expansion data per pad,
//! which is not yet modeled. This stub is registered so the rule
//! infrastructure is in place.

use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking minimum solder mask bridge width.
///
/// Currently a stub — requires solder mask expansion data per pad
/// to compute actual mask-to-mask distances.
pub struct SolderMaskBridgeRule;

impl DrcRule for SolderMaskBridgeRule {
    fn name(&self) -> &'static str {
        "solder-mask-bridge"
    }

    fn check(&self, _world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        // TODO: Implement when solder mask expansion is modeled per pad.
        // Algorithm: for each pair of pads on the same layer within
        // (clearance + 2*mask_expansion) distance, compute the actual
        // mask-to-mask gap and flag if < min_solder_mask_bridge.
        Vec::new()
    }
}
