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
use cypcb_world::components::{FootprintRef, Position, RefDes, Rotation};
use cypcb_world::BoardWorld;

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

                    let (sin, cos) = rotation.to_degrees().to_radians().sin_cos();
                    let (px, py) = (pad.position.x.raw() as f64, pad.position.y.raw() as f64);
                    holes.push(BareHole {
                        entity,
                        centre: Point::new(
                            Nm(position.0.x.raw() + (px * cos - py * sin).round() as i64),
                            Nm(position.0.y.raw() + (px * sin + py * cos).round() as i64),
                        ),
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

                let gap = distance_to_box(hole.centre, entry) - hole.radius.raw();
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

/// Distance in nanometres from a point to the nearest edge of a box, or zero
/// when the point is inside it.
fn distance_to_box(point: Point, entry: &cypcb_world::SpatialEntry) -> i64 {
    let (min_x, min_y) = (entry.envelope.lower()[0], entry.envelope.lower()[1]);
    let (max_x, max_y) = (entry.envelope.upper()[0], entry.envelope.upper()[1]);

    let dx = (min_x - point.x.raw()).max(0).max(point.x.raw() - max_x);
    let dy = (min_y - point.y.raw()).max(0).max(point.y.raw() - max_y);

    if dx == 0 && dy == 0 {
        return 0;
    }
    // Integer hypotenuse: the values are nanometres on a board, so the square
    // fits an i128 with room to spare and the result is exact to the
    // nanometre.
    (((dx as i128 * dx as i128 + dy as i128 * dy as i128) as f64).sqrt()) as i64
}
