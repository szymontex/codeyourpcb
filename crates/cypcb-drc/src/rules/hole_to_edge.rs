//! A hole too near the edge is a hole the router breaks into.
//!
//! The board is cut out of a larger panel by a milling bit that follows the
//! outline. A drilled hole whose wall sits closer to that path than the fab
//! allows comes out of the machine open on one side: a mounting hole with a
//! notch, or a plated hole whose barrel is gone.
//!
//! Not the same question as [`EdgeClearanceRule`], which measures **copper**
//! against the edge. A pad's annulus is wider than the hole inside it, so a
//! pad can clear the edge by the copper rule while its own drill does not -
//! and it is the hole the bit breaks into, not the annulus.
//!
//! `min_hole_to_edge` is one of fifteen numbers every fab preset published
//! with nothing in the workspace reading them. This is the second one closed.

use cypcb_core::Nm;
use cypcb_world::components::{BoardOutline, BoardSize};
use cypcb_world::BoardWorld;

use super::DrcRule;
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// Rule for checking drilled holes against the routed board edge.
pub struct HoleToEdgeRule;

impl DrcRule for HoleToEdgeRule {
    fn name(&self) -> &'static str {
        "hole-to-edge"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let min_gap = rules.min_hole_to_edge.0;

        let Some(board_entity) = world.board_entity() else {
            return Vec::new();
        };
        let Some(board_size) = world.ecs().get::<BoardSize>(board_entity).copied() else {
            return Vec::new();
        };
        let outline = world.ecs().get::<BoardOutline>(board_entity).cloned();

        let mut violations = Vec::new();
        for hole in super::holes_of(world) {
            // The box the hole occupies, so the shared outline distance can
            // measure it: for a circle against a straight cut this is the wall
            // of the hole, and for a milled slot it is both of its ends.
            let (min_x, min_y, max_x, max_y) = hole.bounds();

            let gap = match &outline {
                Some(outline) => {
                    super::edge_clearance::distance_to_outline(outline, min_x, min_y, max_x, max_y)
                }
                None => {
                    let left = min_x;
                    let bottom = min_y;
                    let right = board_size.width.0 - max_x;
                    let top = board_size.height.0 - max_y;
                    left.min(bottom).min(right).min(top)
                }
            };

            if gap < min_gap {
                violations.push(DrcViolation::hole_to_edge(
                    hole.entity,
                    Nm(gap.max(0)),
                    Nm(min_gap),
                    hole.centre(),
                ));
            }
        }

        violations
    }
}
