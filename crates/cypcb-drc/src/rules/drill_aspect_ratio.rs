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
//! A slot is measured by its narrow dimension, which is the bit that makes it
//! and the width the plating has to reach down. Its length is a milling
//! distance, not a depth.
//!
//! `max_drill_aspect_ratio` and `board_thickness` are two of the fifteen
//! numbers every fab preset published with nothing in the workspace reading
//! them. This closes both.

use cypcb_core::Nm;
use cypcb_world::BoardWorld;

use super::DrcRule;
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
        for hole in super::holes_of(world) {
            // A bare hole has no plating to fail, however deep the board.
            if !hole.plated || hole.diameter() >= smallest {
                continue;
            }
            violations.push(DrcViolation::drill_aspect_ratio(
                hole.entity,
                hole.diameter(),
                thickness,
                rules.max_drill_aspect_ratio,
                hole.centre(),
            ));
        }

        violations
    }
}
