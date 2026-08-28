//! An area that hangs off the board.
//!
//! `empty-area` asks whether a declared area is an area. This asks where it
//! is. `region connector_end { bounds 0mm, 0mm to 22mm, 16mm }` is inside a
//! 40mm board and hangs 2mm off a 20mm one, and a `covers connector_end`
//! clause pointing at the second orders a stiffener over air: the handoff
//! document writes a stackup group for a strip of board that is not there.
//!
//! The overhang is measured rather than described. A designer who sees `hangs
//! 2.000mm off the right edge` knows which number to change; one who reads
//! "outside the board" has to work out which of four numbers is wrong and by
//! how much.
//!
//! Two things this deliberately does not do. It does not measure a copper
//! pour: `edge-clearance` already holds copper to the fab's own distance from
//! the edge, and a plane hanging off the board is reported there - a second
//! row for one fault teaches a reader to skim. And it measures against the
//! rectangle `BoardSize` states rather than against a declared outline: an
//! area inside the bounding box but outside a cutout is a question about two
//! polygons, which this rule does not answer and does not pretend to.

use cypcb_world::components::{BoardSize, Zone};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking that a declared area is on the board.
pub struct AreaOffBoardRule;

/// Millimetres, for a message a person reads.
fn mm(value: i64) -> f64 {
    value as f64 / 1_000_000.0
}

impl DrcRule for AreaOffBoardRule {
    fn name(&self) -> &'static str {
        "area-off-board"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let Some(board_entity) = world.board_entity() else {
            return Vec::new();
        };
        let Some(size) = world.ecs().get::<BoardSize>(board_entity).copied() else {
            return Vec::new();
        };
        let (board_w, board_h) = (size.width.0, size.height.0);

        let areas: Vec<(bevy_ecs::entity::Entity, Zone)> = world
            .zones()
            .into_iter()
            .filter(|(_, zone)| !zone.is_copper_pour())
            .collect();

        let mut violations = Vec::new();
        for (entity, zone) in areas {
            let (min_x, min_y) = (zone.bounds.min.x.0, zone.bounds.min.y.0);
            let (max_x, max_y) = (zone.bounds.max.x.0, zone.bounds.max.y.0);

            // One overhang per side, and only the sides it actually hangs off.
            let overhangs = [
                ("left edge", -min_x),
                ("bottom edge", -min_y),
                ("right edge", max_x - board_w),
                ("top edge", max_y - board_h),
            ];
            let Some((side, overhang)) = overhangs
                .iter()
                .filter(|(_, amount)| *amount > 0)
                .max_by_key(|(_, amount)| *amount)
                .copied()
            else {
                continue;
            };

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
            // Hanging off an edge and being nowhere near the board are the
            // same arithmetic and different mistakes, so they read
            // differently.
            let entirely_off = min_x >= board_w || min_y >= board_h || max_x <= 0 || max_y <= 0;
            let how = if entirely_off {
                format!(
                    "sits entirely off a {:.3}mm by {:.3}mm board",
                    mm(board_w),
                    mm(board_h)
                )
            } else {
                format!(
                    "hangs {:.3}mm off the {side} of a {:.3}mm by {:.3}mm board",
                    mm(overhang),
                    mm(board_w),
                    mm(board_h)
                )
            };
            violations.push(DrcViolation::area_off_board(
                entity,
                format!(
                    "the {what} {called} {how}: anything pointing at it - a `covers` clause in \
                     the stackup, the fabricator's document, the 3D view - is being sent to a \
                     part of the panel that is not there"
                ),
                zone.bounds.min,
            ));
        }
        violations
    }
}
