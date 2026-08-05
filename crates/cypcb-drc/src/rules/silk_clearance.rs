//! Silkscreen clearance rule.
//!
//! Silkscreen ink printed over a pad is a real defect: it resists solder, so
//! the joint under it is starved or open. Fabricators either clip the artwork
//! or reject the file.
//!
//! What the board actually prints is what `cypcb-export` puts on the legend
//! layer, and for every component that is the outline of its courtyard - see
//! `gerber::silk::export_silkscreen`. This rule checks exactly that artwork
//! against every other component's pads on the same side.
//!
//! It deliberately does not check a component against its own pads. A
//! footprint's courtyard is drawn around its own copper by construction, and
//! flagging that would fire on every part on the board.

use cypcb_core::{Nm, Point};
use cypcb_world::components::{FootprintRef, Position, RefDes, Rotation};
use cypcb_world::footprint::Footprint;
use cypcb_world::{BoardWorld, Layer};

use crate::presets::DesignRules;
use crate::rules::clearance::segment_distance;
use crate::violation::DrcViolation;

use super::{rotate_point, DrcRule};

/// Width of a silkscreen line, in nanometers.
///
/// Must match `SilkConfig::default().line_width` in `cypcb-export`: this rule
/// measures the ink that crate emits, and a disagreement here means the
/// checker passes a board the fabricator will not.
const SILK_LINE_WIDTH: Nm = Nm(150_000);

/// Rule for checking silkscreen to copper clearance.
pub struct SilkClearanceRule;

impl DrcRule for SilkClearanceRule {
    fn name(&self) -> &'static str {
        "silk-clearance"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let placed = collect_placed(world);
        if placed.len() < 2 {
            return Vec::new();
        }

        let library = world.footprints().clone();
        let half_silk = SILK_LINE_WIDTH.raw() / 2;
        let mut violations = Vec::new();

        for silk in &placed {
            let Some(footprint) = library.get(&silk.footprint) else {
                continue;
            };
            let edges = courtyard_edges(footprint, silk);
            if edges.is_empty() {
                continue;
            }
            let silk_side = side_mask(footprint);

            for other in &placed {
                if other.entity == silk.entity {
                    continue;
                }
                let Some(other_footprint) = library.get(&other.footprint) else {
                    continue;
                };

                if let Some(violation) = first_touch(
                    &edges,
                    silk,
                    silk_side,
                    other,
                    other_footprint,
                    half_silk,
                    rules.min_clearance,
                ) {
                    violations.push(violation);
                }
            }
        }

        violations
    }
}

/// A component as placed on the board.
struct Placed {
    entity: bevy_ecs::entity::Entity,
    refdes: String,
    footprint: String,
    position: Point,
    rotation_deg: f64,
}

fn collect_placed(world: &mut BoardWorld) -> Vec<Placed> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(
        bevy_ecs::entity::Entity,
        &RefDes,
        &FootprintRef,
        &Position,
        &Rotation,
    )>();
    query
        .iter(ecs)
        .map(|(entity, refdes, footprint, position, rotation)| Placed {
            entity,
            refdes: refdes.as_str().to_string(),
            footprint: footprint.as_str().to_string(),
            position: position.0,
            rotation_deg: rotation.to_degrees(),
        })
        .collect()
}

/// The four sides of a placed component's courtyard, in board coordinates.
///
/// Returns nothing for a footprint whose courtyard has no area, which is how
/// the library represents "not known" - there is no artwork to check.
fn courtyard_edges(footprint: &Footprint, placed: &Placed) -> Vec<(Point, Point)> {
    let court = &footprint.courtyard;
    if court.min.x.raw() >= court.max.x.raw() || court.min.y.raw() >= court.max.y.raw() {
        return Vec::new();
    }

    let corners = [
        Point::new(court.min.x, court.min.y),
        Point::new(court.max.x, court.min.y),
        Point::new(court.max.x, court.max.y),
        Point::new(court.min.x, court.max.y),
    ]
    .map(|corner| {
        let rotated = rotate_point(corner, placed.rotation_deg);
        Point::new(
            Nm(placed.position.x.raw() + rotated.x.raw()),
            Nm(placed.position.y.raw() + rotated.y.raw()),
        )
    });

    vec![
        (corners[0], corners[1]),
        (corners[1], corners[2]),
        (corners[2], corners[3]),
        (corners[3], corners[0]),
    ]
}

/// Which board sides a footprint occupies, as a two-bit mask: 1 top, 2 bottom.
///
/// A component's legend is printed on the side it sits on, and the sides its
/// pads reach is the only statement the model makes about that.
fn side_mask(footprint: &Footprint) -> u8 {
    let mut mask = 0u8;
    for pad in &footprint.pads {
        for layer in &pad.layers {
            match layer {
                Layer::TopCopper => mask |= 1,
                Layer::BottomCopper => mask |= 2,
                _ => {}
            }
        }
    }
    // A footprint with no copper at all is a mechanical part; its legend still
    // prints on top by convention.
    if mask == 0 {
        1
    } else {
        mask
    }
}

/// First pad of `other` that the silk artwork runs too close to.
///
/// One violation per pair is enough: a courtyard overlapping a neighbour
/// usually crosses several of its pads, and reporting each one buries the
/// finding.
#[allow(clippy::too_many_arguments)] // each argument is a distinct piece of geometry
fn first_touch(
    edges: &[(Point, Point)],
    silk: &Placed,
    silk_side: u8,
    other: &Placed,
    other_footprint: &Footprint,
    half_silk: i64,
    min_clearance: Nm,
) -> Option<DrcViolation> {
    for pad in &other_footprint.pads {
        let pad_side = pad_side_mask(&pad.layers);
        if pad_side & silk_side == 0 {
            continue;
        }

        let rotated = rotate_point(pad.position, other.rotation_deg);
        let centre = Point::new(
            Nm(other.position.x.raw() + rotated.x.raw()),
            Nm(other.position.y.raw() + rotated.y.raw()),
        );

        // The pad as a disc around its centre, which is what the JavaScript
        // check this replaces used, and conservative for a rectangle.
        let pad_radius = pad.size.0.raw().max(pad.size.1.raw()) / 2;
        let required = pad_radius + half_silk + min_clearance.raw();

        let point = [centre.x.raw(), centre.y.raw()];
        let gap = edges
            .iter()
            .map(|(start, end)| {
                segment_distance(
                    point,
                    point,
                    [start.x.raw(), start.y.raw()],
                    [end.x.raw(), end.y.raw()],
                )
            })
            .min()
            .unwrap_or(i64::MAX);

        if gap < required {
            // Report the distance from ink edge to copper edge, so the number
            // reads the way a fabricator would state it.
            let actual = (gap - pad_radius - half_silk).max(0);
            let mut violation =
                DrcViolation::silk_clearance(silk.entity, Nm(actual), min_clearance, centre)
                    .with_pad_info(&other.refdes, &pad.number);
            violation.message = format!(
                "{} silkscreen over {}.{}: {:.2}mm actual, {:.2}mm required",
                silk.refdes,
                other.refdes,
                pad.number,
                actual as f64 / 1_000_000.0,
                min_clearance.raw() as f64 / 1_000_000.0,
            );
            return Some(violation);
        }
    }
    None
}

/// Which board sides a pad reaches, as the same two-bit mask.
fn pad_side_mask(layers: &[Layer]) -> u8 {
    let mut mask = 0u8;
    for layer in layers {
        match layer {
            Layer::TopCopper => mask |= 1,
            Layer::BottomCopper => mask |= 2,
            _ => {}
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::components::{NetConnections, Value};
    use cypcb_world::footprint::FootprintLibrary;

    /// Two components, `gap_mm` apart centre to centre, both 0402.
    fn board_with_two_parts(gap_mm: f64) -> BoardWorld {
        let mut world = BoardWorld::new();
        world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
        for (index, refdes) in ["R1", "R2"].iter().enumerate() {
            world.spawn_component(
                RefDes::new(*refdes),
                Value::new("10k"),
                Position::from_mm(5.0 + index as f64 * gap_mm, 10.0),
                Rotation::ZERO,
                FootprintRef::new("0402"),
                NetConnections::new(),
            );
        }
        world
    }

    #[test]
    fn an_outline_landing_on_a_neighbours_pad_is_a_defect() {
        // Ink sits on the courtyard outline, not inside it, so the spacing that
        // matters is the one that walks a neighbour's pad onto that line.
        // Computed from the library rather than guessed: an 0402 courtyard is
        // its body plus 0.5mm, and its pads sit half a span either side of
        // centre.
        let library = FootprintLibrary::new();
        let smd = library.get("0402").expect("built-in");
        let half_court = smd.courtyard.max.x.raw() as f64 / 1_000_000.0;
        let half_span = smd.pads[0].position.x.raw().abs() as f64 / 1_000_000.0;

        let mut world = board_with_two_parts(half_court + half_span);
        let violations = SilkClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());

        assert!(
            !violations.is_empty(),
            "an outline printed over a neighbour's pad is a defect"
        );
        assert_eq!(violations[0].kind, crate::ViolationKind::SilkClearance);
        assert!(
            violations[0].message.contains("silkscreen over R"),
            "the message names the part whose ink it is: {}",
            violations[0].message
        );
    }

    #[test]
    fn a_part_is_not_flagged_against_its_own_pads() {
        // One component alone. Its courtyard encloses its own copper by
        // construction, and the previous stub's TODO would have made this fire.
        let mut world = BoardWorld::new();
        world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let violations = SilkClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert!(violations.is_empty(), "got {violations:?}");
    }

    #[test]
    fn an_unknown_footprint_has_no_artwork_to_check() {
        let mut world = BoardWorld::new();
        world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
        for (index, refdes) in ["U1", "U2"].iter().enumerate() {
            world.spawn_component(
                RefDes::new(*refdes),
                Value::new("part"),
                Position::from_mm(5.0 + index as f64 * 0.5, 10.0),
                Rotation::ZERO,
                FootprintRef::new("NOT-IN-THE-LIBRARY"),
                NetConnections::new(),
            );
        }

        let violations = SilkClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert!(violations.is_empty(), "got {violations:?}");
    }

    #[test]
    fn opposite_sides_do_not_interfere() {
        let library = FootprintLibrary::new();
        let smd = library.get("0402").expect("built-in");
        assert_eq!(side_mask(smd), 1, "an 0402 is a top-side part");

        let tht = library.get("DIP-8").or_else(|| library.get("PIN-HDR-1x2"));
        if let Some(tht) = tht {
            assert_eq!(
                side_mask(tht) & 3,
                3,
                "a through-hole part reaches both sides"
            );
        }
    }
}
