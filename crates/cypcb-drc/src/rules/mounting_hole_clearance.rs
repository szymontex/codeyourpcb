//! Copper too close to a hole that is not plated.
//!
//! Every other rule that walks pad copper walks straight past a mounting hole,
//! because a mounting hole has no copper. The courtyard rule stops a *part*
//! being placed on one; nothing stopped a *trace* being drawn across one.
//!
//! The autorouter will not do it - the grid blocks the hole on every layer -
//! but the router is not the only way copper reaches a board. A trace drawn by
//! hand, a board imported from KiCad, or a zone poured over the hole all
//! arrive without the router's opinion.
//!
//! When it is missed, the drill cuts the trace - so the net is open - and the
//! copper it exposes at the hole wall touches the screw. A metal standoff then
//! ties that net to the chassis.

use cypcb_core::{Nm, Point};
use cypcb_world::components::{place_pad, FootprintRef, Position, RefDes, Rotation};
use cypcb_world::BoardWorld;

use super::clearance::{point_to_segment_distance, Copper, EntryCopper, Piece};
use super::DrcRule;
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// One hole with no copper, in board coordinates.
struct BareHole {
    /// The component the hole belongs to, so it is not measured against its
    /// own entry in the spatial index. The index gives a mounting hole an
    /// envelope with copper layers set even though its pad has none, so the
    /// first version of this rule reported every hole against itself at a
    /// distance of zero - on a board with no copper near any hole at all.
    entity: cypcb_world::Entity,
    centre: Point,
    radius: Nm,
    refdes: String,
}

/// Rule that keeps copper away from the wall of an unplated hole.
///
/// Measured with `min_edge_clearance`, because that is what such a hole is: a
/// board edge cut into the middle of the board. The same drill exposes the
/// same copper for the same reason, and the fabricator's number for it is
/// already in the rule set - inventing a second one would mean inventing a
/// value no board house published.
///
/// Reported as `EdgeClearance` for the same reason, with a message that names
/// the hole so the reader is not left looking at the board outline.
pub struct MountingHoleClearanceRule;

impl DrcRule for MountingHoleClearanceRule {
    fn name(&self) -> &'static str {
        "mounting-hole-clearance"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let required = rules.min_edge_clearance;

        let library = world.footprints().clone();

        // Where the bare holes are. Collected first so the ECS borrow ends
        // before the spatial index is read.
        let mut holes: Vec<BareHole> = Vec::new();
        {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(
                cypcb_world::Entity,
                &RefDes,
                &Position,
                &Rotation,
                &FootprintRef,
            )>();
            for (entity, refdes, position, rotation, footprint_ref) in query.iter(ecs) {
                let Some(footprint) = library.get(&footprint_ref.0) else {
                    continue;
                };
                for pad in &footprint.pads {
                    if !pad.is_non_plated() {
                        continue;
                    }
                    let Some(drill) = pad.drill else { continue };

                    holes.push(BareHole {
                        entity,
                        centre: place_pad(position.0, pad.position, *rotation),
                        radius: Nm(drill.raw() / 2),
                        refdes: refdes.as_str().to_string(),
                    });
                }
            }
        }

        if holes.is_empty() {
            return violations;
        }

        // Every piece of copper on the board. Zones are added by hand for the
        // same reason the edge rule adds them: a pour is not in the spatial
        // index, and a plane poured across a mounting hole is exactly the case
        // worth catching.
        // A component sits in the spatial index as its **courtyard**, and a
        // courtyard is not copper. Measuring that box reported a part whose
        // plastic body reaches over a mounting hole while its pads stay well
        // clear - the drill cuts nothing of that part. `ClearanceRule` and the
        // edge rule both measure pads through this collector; so does this one
        // now.
        // A trace segment and a via are measured the same way: a trace is its
        // centreline grown by half its width and a via is its disc, not the
        // box the index holds for either.
        let copper = EntryCopper::collect(world);

        let mut entries: Vec<cypcb_world::SpatialEntry> = world.spatial().iter().cloned().collect();
        for (entity, zone) in world.zones() {
            if zone.is_keepout() {
                continue;
            }
            entries.push(cypcb_world::SpatialEntry::new(
                entity,
                zone.bounds.min,
                zone.bounds.max,
                zone.layer_mask,
            ));
        }

        for hole in &holes {
            for entry in &entries {
                // A hole is drilled through the whole board, so which layers
                // the copper is on does not matter - only whether it is copper
                // at all. `layer_mask == 0` is how the hole's own pad appears,
                // and measuring a hole against itself would report every hole
                // on the board.
                if entry.layer_mask == 0 || entry.entity == hole.entity {
                    continue;
                }

                // The copper this entry stands for, as `EntryCopper` gives it.
                let gap = copper
                    .pieces(entry)
                    .iter()
                    .map(|piece| distance_to_piece(hole.centre, piece))
                    .min()
                    .unwrap_or(i64::MAX)
                    - hole.radius.raw();
                if gap < required.raw() {
                    violations.push(DrcViolation::edge_clearance(
                        entry.entity,
                        Nm(gap.max(0)),
                        required,
                        hole.centre,
                    ));
                    let last = violations.len() - 1;
                    violations[last].message = format!(
                        "Copper too close to unplated hole {}: {:.2}mm actual, {:.2}mm required. \
                         The drill cuts this copper open and the screw touches what is left.",
                        hole.refdes,
                        Nm(gap.max(0)).to_mm(),
                        required.to_mm(),
                    );
                }
            }
        }

        violations
    }
}

/// Distance in nanometres from a point to one piece of copper: to its core or
/// its centreline, less the radius it is grown by.
fn distance_to_piece(point: Point, piece: &Piece) -> i64 {
    match *piece {
        Piece::Area(copper) => distance_to_copper(point, &copper),
        Piece::Stroke { from, to, radius } => {
            let to_axis = point_to_segment_distance([point.x.raw(), point.y.raw()], from, to);
            (to_axis - radius).max(0)
        }
    }
}

/// Distance in nanometres from a point to a box grown by a radius: to its
/// core, less the radius.
fn distance_to_copper(point: Point, copper: &Copper) -> i64 {
    let (min_x, min_y) = (copper.core.lower()[0], copper.core.lower()[1]);
    let (max_x, max_y) = (copper.core.upper()[0], copper.core.upper()[1]);

    let dx = (min_x - point.x.raw()).max(0).max(point.x.raw() - max_x);
    let dy = (min_y - point.y.raw()).max(0).max(point.y.raw() - max_y);

    let to_core = (((dx as i128 * dx as i128 + dy as i128 * dy as i128) as f64).sqrt()) as i64;
    (to_core - copper.radius).max(0)
}
