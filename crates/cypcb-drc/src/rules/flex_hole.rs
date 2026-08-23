//! A plated hole in a bend cracks.
//!
//! `flex 20mm, 0mm to 30mm, 20mm` is the part of a rigid-flex board that bends
//! in service. The barrel of a plated hole is copper - a tube of it, plated
//! onto the wall - and the laminate around it moves every time the board is
//! folded. The barrel does not: it work-hardens and splits, usually at the
//! knee where the plating meets the pad, and usually after the product has
//! shipped.
//!
//! Every flex design guide says the same thing in the same words: no holes in
//! the bend. This says it about a specific hole, at its coordinates, before
//! the board is ordered.
//!
//! What counts as a hole here is anything drilled and plated: a via, and a
//! through-hole pad. A mounting hole is drilled and not plated, and it is
//! still reported - a hole of any kind is a discontinuity in a strip that is
//! being bent, and the ones that are not plated tear the laminate rather than
//! the copper.

use cypcb_world::components::zone::Zone;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::{holes_of, DrcRule};

/// Rule for checking that nothing is drilled where the board bends.
pub struct FlexHoleRule;

impl DrcRule for FlexHoleRule {
    fn name(&self) -> &'static str {
        "flex-hole"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let regions: Vec<Zone> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&Zone>();
            query
                .iter(ecs)
                .filter(|zone| zone.is_flex())
                .cloned()
                .collect()
        };
        // Most boards do not bend, and this rule says nothing about them.
        if regions.is_empty() {
            return Vec::new();
        }

        let holes = holes_of(world);
        let mut violations = Vec::new();
        for hole in holes {
            let Some(region) = regions.iter().find(|region| region.contains(hole.start)) else {
                continue;
            };
            let where_it_is = match &region.name {
                Some(name) => format!("the flexible region '{name}'"),
                None => "a flexible region".to_string(),
            };
            violations.push(DrcViolation::flex_hole(
                hole.entity,
                format!(
                    "a hole {:.3}mm across sits in {where_it_is}: the barrel is copper and the \
                     laminate around it moves, so a plated hole in a bend cracks",
                    hole.radius as f64 * 2.0 / 1_000_000.0
                ),
                hole.start,
            ));
        }
        violations
    }
}
