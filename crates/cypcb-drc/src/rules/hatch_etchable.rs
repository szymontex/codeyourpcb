//! A mesh the fabricator cannot etch.
//!
//! `hatch 0.3mm pitch 1mm` is two figures the filler turns into copper: lines
//! that wide, that far apart centre to centre. Both are held to the same
//! numbers every other piece of copper on the board is held to - the line is a
//! trace as far as the etch bath is concerned, and the space between two lines
//! is clearance - and until this rule nothing compared them.
//!
//! `hatch 0.05mm pitch 0.06mm` describes a mesh JLCPCB will not make: 0.05mm
//! of copper where the table says 0.127mm, and 0.01mm of gap where it says
//! 0.127mm. The design would have gone to the fab, and the fab would have said
//! so - after the money.
//!
//! The gap is derived rather than stated, which is why this reads the pitch
//! and not just the width: pitch minus width is the space between one line and
//! the next, and a design that states a pitch barely wider than its lines has
//! asked for copper with slivers of laminate in it.

use cypcb_world::components::{Hatch, Zone};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for a hatch finer than the house etches.
pub struct HatchEtchableRule;

impl DrcRule for HatchEtchableRule {
    fn name(&self) -> &'static str {
        "hatch-too-fine"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let meshed: Vec<(bevy_ecs::entity::Entity, Zone, Hatch)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Zone, &Hatch)>();
            query
                .iter(ecs)
                .map(|(entity, zone, hatch)| (entity, zone.clone(), *hatch))
                .collect()
        };

        let mut violations = Vec::new();
        for (entity, zone, hatch) in meshed {
            let named = match &zone.name {
                Some(name) => format!("the pour '{name}'"),
                None => "a pour".to_string(),
            };

            if hatch.width < rules.min_trace_width {
                violations.push(DrcViolation::hatch_too_fine(
                    entity,
                    hatch.width,
                    rules.min_trace_width,
                    format!(
                        "{named} is hatched with {:.3}mm lines and this house etches {:.3}mm: a \
                         line of a mesh is copper like any other, and one thinner than the fab \
                         holds comes back broken or not at all",
                        hatch.width.to_mm(),
                        rules.min_trace_width.to_mm()
                    ),
                    zone.bounds.min,
                ));
            }

            // The gap is what the design did not state: pitch minus width is
            // the laminate between one line and the next, and it is held to
            // the same clearance as any other space between two pieces of
            // copper.
            let gap = hatch.pitch - hatch.width;
            if hatch.pitch > hatch.width && gap < rules.min_clearance {
                violations.push(DrcViolation::hatch_too_fine(
                    entity,
                    gap,
                    rules.min_clearance,
                    format!(
                        "{named} leaves {:.3}mm between its lines - {:.3}mm pitch less {:.3}mm of \
                         copper - and this house holds {:.3}mm: a gap the etch cannot open is a \
                         plane poured solid by accident",
                        gap.to_mm(),
                        hatch.pitch.to_mm(),
                        hatch.width.to_mm(),
                        rules.min_clearance.to_mm()
                    ),
                    zone.bounds.min,
                ));
            }

            // A pitch no wider than the lines is not a mesh at all. The filler
            // says so by leaving the plane solid; this says why, because a
            // designer who wrote it meant to hatch something.
            if hatch.pitch <= hatch.width {
                violations.push(DrcViolation::hatch_too_fine(
                    entity,
                    hatch.pitch,
                    hatch.width,
                    format!(
                        "{named} states a {:.3}mm pitch and {:.3}mm lines, so its lines touch: \
                         that is a sheet of copper rather than a mesh, and the filler leaves it \
                         solid",
                        hatch.pitch.to_mm(),
                        hatch.width.to_mm()
                    ),
                    zone.bounds.min,
                ));
            }
        }
        violations
    }
}
