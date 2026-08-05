//! Courtyard clearance rule.
//!
//! Checks that component courtyards don't overlap or are too close.
//! This prevents physical interference during assembly.

use cypcb_core::{Nm, Point};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking minimum courtyard clearance between components.
pub struct CourtyardClearanceRule;

impl DrcRule for CourtyardClearanceRule {
    fn name(&self) -> &'static str {
        "courtyard-clearance"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_clearance = rules.min_courtyard_clearance;

        // Collect all positioned components with their courtyards from spatial index.
        // We use the spatial entries with layer_mask == 0 which are courtyard entries.
        let entries: Vec<_> = world.spatial().iter().cloned().collect();

        // Filter to courtyard entries (layer_mask == 0, set in rebuild_spatial_index_full)
        let courtyards: Vec<_> = entries.iter().filter(|e| e.layer_mask == 0).collect();

        // Check all pairs of courtyard AABBs
        for i in 0..courtyards.len() {
            for j in (i + 1)..courtyards.len() {
                let a = courtyards[i];
                let b = courtyards[j];

                // Skip if same entity
                if a.entity == b.entity {
                    continue;
                }

                // AABB gap in each dimension
                let dx = (a.envelope.lower()[0].max(b.envelope.lower()[0])
                    - a.envelope.upper()[0].min(b.envelope.upper()[0]))
                .max(0);
                let dy = (a.envelope.lower()[1].max(b.envelope.lower()[1])
                    - a.envelope.upper()[1].min(b.envelope.upper()[1]))
                .max(0);

                let distance = if dx == 0 && dy == 0 {
                    0 // Overlapping
                } else {
                    let dx_sq = (dx as i128) * (dx as i128);
                    let dy_sq = (dy as i128) * (dy as i128);
                    ((dx_sq + dy_sq) as f64).sqrt() as i64
                };

                if distance < min_clearance.0 {
                    let location = Point::new(
                        Nm((a.envelope.lower()[0] + a.envelope.upper()[0]) / 2),
                        Nm((a.envelope.lower()[1] + a.envelope.upper()[1]) / 2),
                    );
                    violations.push(DrcViolation::courtyard_clearance(
                        a.entity,
                        b.entity,
                        Nm(distance),
                        min_clearance,
                        location,
                    ));
                }
            }
        }

        violations
    }
}
