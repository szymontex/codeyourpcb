//! A plane poured solid across a fold.
//!
//! IPC-2223 asks for a hatched polygon in the flex area rather than solid
//! copper. A sheet of copper over a fold is the same failure as a trace
//! running along one, at the width of the whole plane: the copper takes the
//! strain of the bend across an unbroken surface, and it cracks at the line
//! where the fold begins. A hatch is a mesh of narrow copper with gaps between
//! it, so the sheet can move.
//!
//! `copper::fill_zone` fills a pour solid and knows nothing about bends, so
//! this reports the overlap rather than fixing it: what a plane in a bend
//! should be filled with is a change to the filler, and a checker that quietly
//! hatched a plane the design asked for would be answering a question nobody
//! put to it.
//!
//! The overlap is measured, the way `area-overlap` measures a contested strip.
//! A designer who reads `4.000mm by 16.000mm` knows how much of the plane is
//! in the fold; one who reads "overlaps the bend" has to work it out.

use cypcb_core::{Nm, Point};
use cypcb_world::components::Zone;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for a copper pour that crosses a flexible region.
pub struct SolidPourInBendRule;

/// Millimetres, for a message a person reads.
fn mm(value: i64) -> f64 {
    value as f64 / 1_000_000.0
}

impl DrcRule for SolidPourInBendRule {
    fn name(&self) -> &'static str {
        "solid-pour-in-bend"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let zones: Vec<(bevy_ecs::entity::Entity, Zone)> = world.zones().into_iter().collect();
        let folds: Vec<&(bevy_ecs::entity::Entity, Zone)> =
            zones.iter().filter(|(_, zone)| zone.is_flex()).collect();
        // Most boards do not bend, and this rule says nothing about them.
        if folds.is_empty() {
            return Vec::new();
        }

        let mut violations = Vec::new();
        for (entity, pour) in zones.iter().filter(|(_, zone)| zone.is_copper_pour()) {
            for (fold_entity, fold) in &folds {
                // The two have to share copper, not only ground: a plane on
                // the top layer and a ribbon stated on the bottom are not over
                // each other in any sense a fabricator cares about.
                if pour.layer_mask & fold.layer_mask == 0 {
                    continue;
                }

                let min_x = pour.bounds.min.x.0.max(fold.bounds.min.x.0);
                let min_y = pour.bounds.min.y.0.max(fold.bounds.min.y.0);
                let max_x = pour.bounds.max.x.0.min(fold.bounds.max.x.0);
                let max_y = pour.bounds.max.y.0.min(fold.bounds.max.y.0);
                let (width, height) = (max_x - min_x, max_y - min_y);
                if width <= 0 || height <= 0 {
                    continue;
                }

                let poured = match &pour.name {
                    Some(name) => format!("the pour '{name}'"),
                    None => "a pour".to_string(),
                };
                let bends = match &fold.name {
                    Some(name) => format!("'{name}'"),
                    None => "a flexible region".to_string(),
                };
                violations.push(DrcViolation::solid_pour_in_bend(
                    *entity,
                    *fold_entity,
                    format!(
                        "{poured} covers {:.3}mm by {:.3}mm of {bends} from ({:.3}mm, {:.3}mm) \
                         and this tool fills a pour solid: a sheet of copper over a fold takes \
                         the strain across an unbroken surface and cracks where the fold \
                         begins, which is why IPC-2223 asks for a hatched polygon in a flex \
                         area",
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
