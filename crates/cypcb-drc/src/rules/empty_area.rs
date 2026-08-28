//! An area with no area at all.
//!
//! A design names an area to point at it: `region connector_end { bounds 0mm,
//! 0mm to 22mm, 16mm }`, and then `stiffener 0.2mm covers connector_end`. Every
//! reader downstream takes that rectangle at its word - the handoff document
//! writes a stackup group for it, the 3D view asks whether a layer is inside
//! it, and the copper filler cuts a pour to it.
//!
//! A rectangle whose two corners share an edge has no width or no height, so
//! it contains nothing: no pad is in it, no layer stops at it, and the stackup
//! group written for it describes a strip of board 0mm wide. Nothing refused
//! it, because nothing looked - `bounds 10mm, 5mm to 10mm, 25mm` is four
//! numbers like any other and the typo that produces it is one keystroke.
//!
//! Every kind of area is asked the same question. A pour with no area fills
//! nothing, a keepout with no area keeps nothing out, a flexible region with
//! no area bends nowhere, and a named area with no area is a name pointing at
//! a rectangle that is not there.

use cypcb_world::components::zone::Zone;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking that a declared area is an area.
pub struct EmptyAreaRule;

impl DrcRule for EmptyAreaRule {
    fn name(&self) -> &'static str {
        "empty-area"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let areas: Vec<(bevy_ecs::entity::Entity, Zone)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Zone)>();
            query
                .iter(ecs)
                .map(|(entity, zone)| (entity, zone.clone()))
                .collect()
        };

        let mut violations = Vec::new();
        for (entity, zone) in areas {
            let width = zone.bounds.max.x.0 - zone.bounds.min.x.0;
            let height = zone.bounds.max.y.0 - zone.bounds.min.y.0;
            if width > 0 && height > 0 {
                continue;
            }

            // The word the design used for it, so the message reads back as
            // the line that produced it rather than as a kind of shape.
            let what = match zone.kind {
                cypcb_world::components::ZoneKind::CopperPour => "pour",
                cypcb_world::components::ZoneKind::Keepout => "keepout",
                cypcb_world::components::ZoneKind::Flex => "flexible region",
                cypcb_world::components::ZoneKind::Region => "named area",
            };
            let called = match &zone.name {
                Some(name) => format!("'{name}'"),
                None => "with no name".to_string(),
            };
            // Which of the two collapsed, because a designer looking for the
            // typo is looking at one pair of numbers rather than four.
            let how = if width <= 0 && height <= 0 {
                "no width and no height".to_string()
            } else if width <= 0 {
                format!(
                    "no width: it runs {:.3}mm down and 0mm across",
                    height as f64 / 1_000_000.0
                )
            } else {
                format!(
                    "no height: it runs {:.3}mm across and 0mm down",
                    width as f64 / 1_000_000.0
                )
            };
            violations.push(DrcViolation::empty_area(
                entity,
                format!(
                    "the {what} {called} has {how}, so it contains nothing: every reader of it \
                     - the fabricator's document, the 3D view, the copper filler - is being \
                     pointed at a rectangle that is not there"
                ),
                zone.bounds.min,
            ));
        }
        violations
    }
}
