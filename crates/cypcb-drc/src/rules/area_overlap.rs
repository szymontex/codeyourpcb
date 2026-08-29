//! Two areas the stack points at, over the same strip of board.
//!
//! A rigid-flex build is several stacks on one panel, and each is stated
//! against an area: `core 1mm covers left`, `core 0.5mm covers right`. Where
//! two such areas overlap, both stacks describe the same strip - and the
//! handoff document writes a `StackupGroup` for each, so a fabricator reading
//! it is told the board is two thicknesses in one place. There is no right
//! answer to pick: which stack owns the contested strip is a decision only the
//! designer can make, the same way `zone-overlap` refuses to pick between two
//! planes over one patch of copper.
//!
//! Only areas a stackup layer points at are measured. Two areas that nothing
//! in the stack names may overlap freely - a flexible region inside a named
//! end is an ordinary thing to draw, and reporting it would be noise about a
//! fact no document carries.

use cypcb_core::{Nm, Point};
use cypcb_world::components::Zone;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for two stack-bearing areas that overlap.
pub struct AreaOverlapRule;

/// Millimetres, for a message a person reads.
fn mm(value: i64) -> f64 {
    value as f64 / 1_000_000.0
}

impl DrcRule for AreaOverlapRule {
    fn name(&self) -> &'static str {
        "area-overlap"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let Some(stack) = world.stackup().cloned() else {
            return Vec::new();
        };
        let named = stack.areas();
        if named.len() < 2 {
            return Vec::new();
        }

        // Only the areas the stack points at, in the order the stack names
        // them, so two boards that differ only in the order of their `region`
        // blocks report the same pair the same way round.
        let areas: Vec<(bevy_ecs::entity::Entity, Zone)> = named
            .iter()
            .filter_map(|name| {
                world
                    .zones()
                    .into_iter()
                    .find(|(_, zone)| zone.name.as_deref() == Some(name.as_str()))
            })
            .collect();

        let mut violations = Vec::new();
        for i in 0..areas.len() {
            for j in (i + 1)..areas.len() {
                let (entity, first) = &areas[i];
                let (other_entity, second) = &areas[j];

                let min_x = first.bounds.min.x.0.max(second.bounds.min.x.0);
                let min_y = first.bounds.min.y.0.max(second.bounds.min.y.0);
                let max_x = first.bounds.max.x.0.min(second.bounds.max.x.0);
                let max_y = first.bounds.max.y.0.min(second.bounds.max.y.0);
                let (width, height) = (max_x - min_x, max_y - min_y);
                if width <= 0 || height <= 0 {
                    continue;
                }

                let first_name = first.name.clone().unwrap_or_default();
                let second_name = second.name.clone().unwrap_or_default();
                // What each stack says the board is there, when both say a
                // number. A stack with a layer that states no thickness
                // answers `None`, and this says so rather than printing a
                // partial sum as if it were a measurement.
                let thicknesses = match (
                    stack.thickness_in_area(&first_name),
                    stack.thickness_in_area(&second_name),
                ) {
                    (Some(one), Some(two)) if one != two => format!(
                        ", where one stack is {:.3}mm thick and the other {:.3}mm",
                        mm(one.0),
                        mm(two.0)
                    ),
                    _ => String::new(),
                };

                violations.push(DrcViolation::area_overlap(
                    *entity,
                    *other_entity,
                    format!(
                        "'{first_name}' and '{second_name}' both cover {:.3}mm by {:.3}mm of \
                         board from ({:.3}mm, {:.3}mm){thicknesses}: the stack states one build \
                         per area and the handoff document writes a group for each, so this \
                         strip is described twice",
                        mm(width),
                        mm(height),
                        mm(min_x),
                        mm(min_y)
                    ),
                    Point::new(Nm(min_x), Nm(min_y)),
                ));
            }
        }
        violations
    }
}
