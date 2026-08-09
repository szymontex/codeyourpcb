//! Silkscreen clearance rule.
//!
//! Silkscreen ink printed over a pad is a real defect: it resists solder, so
//! the joint under it is starved or open. Fabricators either clip the artwork
//! or reject the file.
//!
//! What the board actually prints is what `cypcb-export` puts on the legend
//! layer - see `gerber::silk::export_silkscreen`. For every component that is
//! its artwork, or its courtyard outline when it has none, plus its designator
//! drawn as strokes. This rule measures all of it.
//!
//! The outline is checked against every *other* component's pads only. A
//! footprint's courtyard is drawn around its own copper by construction, and
//! flagging that would fire on every part on the board.
//!
//! The designator is checked against its own pads as well, because nothing
//! guarantees it clears them: it is laid out from the part's origin by
//! `cypcb_world::silk_text`, which knows the text height and nothing about the
//! footprint under it. A name printed across a part's own pad starves that
//! joint exactly as a neighbour's would.
//!
//! # What "actually prints" means since the exporter started clipping
//!
//! `cypcb-export` cuts the legend off solderable copper the way a board house
//! does, at the clearance of the fabricator it was told about. So this rule
//! measures the clipped artwork, not the intent - it is a check on the file
//! rather than on the layout. That is not a rule with nothing to do: it fires
//! whenever the house the board is *checked* for wants more clearance than the
//! one it was *clipped* for, which is a legend that looked fine on the way out
//! and does not meet the spec of the shop it arrived at.

use cypcb_core::{Nm, Point};
use cypcb_world::components::{FootprintRef, Position, RefDes, Rotation, Side};
use cypcb_world::footprint::{Footprint, SilkShape};
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

/// How tall a printed designator is, in nanometers.
///
/// Must match `SilkConfig::default().text_height` in `cypcb-export`, for the
/// same reason as the line width above.
const DESIGNATOR_HEIGHT: Nm = Nm(1_000_000);

/// The clearance the exporter clips the legend to, in nanometers.
///
/// Must match `SilkConfig::default().clearance`. This rule measures the ink
/// that survives that clipping against the rules of the fabricator the board
/// is *checked* for, so the two numbers differing is not a bug here - it is
/// the case this rule exists to catch: a legend clipped for one house and sent
/// to a stricter one.
const EXPORT_CLEARANCE: Nm = Nm(130_000);

/// What is left of some artwork after the legend is clipped off the copper.
fn clipped(
    edges: Vec<(Point, Point)>,
    keepouts: &[cypcb_world::silk_text::Keepout],
) -> Vec<(Point, Point)> {
    let shapes = edges
        .into_iter()
        .map(|(start, end)| SilkShape::Segment {
            start,
            end,
            width: SILK_LINE_WIDTH,
        })
        .collect();

    cypcb_world::silk_text::clip_strokes(shapes, keepouts)
        .into_iter()
        .filter_map(|shape| match shape {
            SilkShape::Segment { start, end, .. } => Some((start, end)),
            SilkShape::Circle { .. } => None,
        })
        .collect()
}

/// Rule for checking silkscreen to copper clearance.
pub struct SilkClearanceRule;

impl DrcRule for SilkClearanceRule {
    fn name(&self) -> &'static str {
        "silk-clearance"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        // One part is enough: its own name can land on its own pads.
        let placed = collect_placed(world);
        if placed.is_empty() {
            return Vec::new();
        }

        let library = world.footprints().clone();
        let half_silk = SILK_LINE_WIDTH.raw() / 2;

        // What the exporter will actually print: it clips the legend off
        // solderable copper the way a board house does, using the clearance of
        // the fabricator it was told about. This rule measures what survives
        // that, so it stays a check on the file rather than on the intent -
        // and it fires when the two disagree, which is the case worth
        // catching: a board clipped for one fabricator and checked against a
        // stricter one.
        let clip_margin = Nm(EXPORT_CLEARANCE.raw() + half_silk);
        let top_keepouts =
            cypcb_world::silk_text::pad_keepouts(world, &library, Layer::TopCopper, clip_margin);
        let bottom_keepouts =
            cypcb_world::silk_text::pad_keepouts(world, &library, Layer::BottomCopper, clip_margin);

        // Where each part's copper is, so the loop below can ask what is near
        // a legend instead of asking every part in turn. Built here rather
        // than taken from the world's spatial index, because a caller that has
        // not rebuilt that index would get an empty answer and this rule would
        // quietly stop checking - the failure mode this project has already
        // been bitten by twice.
        let boxes: Vec<(Point, Point)> = placed
            .iter()
            .map(|part| match library.get(&part.footprint) {
                Some(footprint) => pad_bounds(footprint, part),
                None => (part.position, part.position),
            })
            .collect();
        let reach = Nm(rules.min_clearance.raw() + half_silk);
        let parts_near = Grid::build(&boxes);

        // The same treatment for the clipping. `clip_strokes` walks every
        // keepout it is handed, and the board's keepouts are every pad on the
        // layer - so clipping one part's legend against all of them was the
        // other half of this rule's quadratic cost.
        let top_near = Grid::build(&keepout_boxes(&top_keepouts));
        let bottom_near = Grid::build(&keepout_boxes(&bottom_keepouts));

        let mut violations = Vec::new();

        for silk in &placed {
            let Some(footprint) = library.get(&silk.footprint) else {
                continue;
            };
            // The part says which face it is on when the model knows; falling
            // back to its copper is a guess that cannot tell a bottom-side
            // through-hole part from a top-side one.
            let silk_side = silk
                .side
                .map_or_else(|| side_mask(footprint), |s| s.mask() as u8);

            // The legend is clipped against the copper on the face it prints
            // on, which is the face the exporter clips against too.
            let (clip_against, clip_near) = if silk_side & 2 != 0 {
                (&bottom_keepouts, &bottom_near)
            } else {
                (&top_keepouts, &top_near)
            };

            // The name is checked against every part including this one; the
            // outline only against the others. Both as printed, not as laid
            // out.
            let raw_name = designator_edges(footprint, silk);
            let raw_edges = silk_segments(footprint, silk);

            // Only the copper this legend could be clipped by. The keepouts
            // already carry the exporter's clearance in their own size, so the
            // ink's own box is the right question to ask.
            let mut all = raw_name.clone();
            all.extend_from_slice(&raw_edges);
            let nearby: Vec<cypcb_world::silk_text::Keepout> = if all.is_empty() {
                Vec::new()
            } else {
                let (min, max) = bounds_of(&all, Nm(0));
                clip_near
                    .overlapping(min, max)
                    .into_iter()
                    .map(|index| clip_against[index])
                    .collect()
            };

            let name = clipped(raw_name, &nearby);
            let mut edges = clipped(raw_edges, &nearby);
            edges.extend_from_slice(&name);
            if edges.is_empty() {
                continue;
            }

            // Only the parts whose copper could be near this legend. This
            // loop used to be every part against every part: on a 400-part
            // board that is 160,000 pairs of segment-against-pad geometry, and
            // it made this one rule the entire cost of loading a board -
            // 469ms of a 447ms design rule check, quadratic, while every other
            // rule stayed under a millisecond.
            let (min, max) = bounds_of(&edges, reach);
            for index in parts_near.overlapping(min, max) {
                let other = &placed[index];
                let Some(other_footprint) = library.get(&other.footprint) else {
                    continue;
                };
                let against: &[(Point, Point)] = if other.entity == silk.entity {
                    &name
                } else {
                    &edges
                };
                if against.is_empty() {
                    continue;
                }

                if let Some(violation) = first_touch(
                    against,
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

/// A uniform grid over boxes, so a loop can ask what is near something.
///
/// Not the world's spatial index on purpose: that one is rebuilt by the
/// caller, and a rule that silently checks nothing when somebody forgets is
/// worse than a rule that costs a little to set up.
struct Grid {
    cell: i64,
    buckets: std::collections::HashMap<(i64, i64), Vec<usize>>,
}

impl Grid {
    fn build(boxes: &[(Point, Point)]) -> Self {
        // One cell as wide as the widest thing in it, so nothing spans more
        // than two cells on an axis and the buckets stay small.
        let widest = boxes
            .iter()
            .map(|(min, max)| (max.x.0 - min.x.0).max(max.y.0 - min.y.0))
            .max()
            .unwrap_or(0);
        let cell = widest.max(1_000_000); // never finer than a millimetre

        let mut buckets: std::collections::HashMap<(i64, i64), Vec<usize>> =
            std::collections::HashMap::new();
        for (index, (min, max)) in boxes.iter().enumerate() {
            for cx in min.x.0.div_euclid(cell)..=max.x.0.div_euclid(cell) {
                for cy in min.y.0.div_euclid(cell)..=max.y.0.div_euclid(cell) {
                    buckets.entry((cx, cy)).or_default().push(index);
                }
            }
        }

        Grid { cell, buckets }
    }

    /// Every box that might overlap this region, each once.
    fn overlapping(&self, min: Point, max: Point) -> Vec<usize> {
        let mut found = Vec::new();
        for cx in min.x.0.div_euclid(self.cell)..=max.x.0.div_euclid(self.cell) {
            for cy in min.y.0.div_euclid(self.cell)..=max.y.0.div_euclid(self.cell) {
                if let Some(bucket) = self.buckets.get(&(cx, cy)) {
                    found.extend_from_slice(bucket);
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        found
    }
}

/// The box each keepout covers.
fn keepout_boxes(keepouts: &[cypcb_world::silk_text::Keepout]) -> Vec<(Point, Point)> {
    keepouts
        .iter()
        .map(|keepout| {
            let half = keepout.half_size.raw();
            (
                Point::new(Nm(keepout.centre.x.0 - half), Nm(keepout.centre.y.0 - half)),
                Point::new(Nm(keepout.centre.x.0 + half), Nm(keepout.centre.y.0 + half)),
            )
        })
        .collect()
}

/// The box a part's pads cover on the board.
fn pad_bounds(footprint: &Footprint, part: &Placed) -> (Point, Point) {
    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;
    for pad in &footprint.pads {
        let centre = rotate_point(pad.position, part.rotation_deg);
        let x = part.position.x.0 + centre.x.0;
        let y = part.position.y.0 + centre.y.0;
        // A rotated pad is bounded by its own diagonal, whatever the angle.
        let reach = (pad.size.0.raw().max(pad.size.1.raw()) + 1) / 2;
        min_x = min_x.min(x - reach);
        min_y = min_y.min(y - reach);
        max_x = max_x.max(x + reach);
        max_y = max_y.max(y + reach);
    }
    if min_x == i64::MAX {
        return (part.position, part.position);
    }
    (
        Point::new(Nm(min_x), Nm(min_y)),
        Point::new(Nm(max_x), Nm(max_y)),
    )
}

/// The box a set of edges covers, grown by `margin` on every side.
fn bounds_of(edges: &[(Point, Point)], margin: Nm) -> (Point, Point) {
    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;
    for (start, end) in edges {
        min_x = min_x.min(start.x.0).min(end.x.0);
        min_y = min_y.min(start.y.0).min(end.y.0);
        max_x = max_x.max(start.x.0).max(end.x.0);
        max_y = max_y.max(start.y.0).max(end.y.0);
    }
    (
        Point::new(Nm(min_x - margin.raw()), Nm(min_y - margin.raw())),
        Point::new(Nm(max_x + margin.raw()), Nm(max_y + margin.raw())),
    )
}

/// A component as placed on the board.
struct Placed {
    entity: bevy_ecs::entity::Entity,
    refdes: String,
    footprint: String,
    position: Point,
    rotation_deg: f64,
    /// The face this part is mounted on, when the model states it.
    side: Option<Side>,
}

fn collect_placed(world: &mut BoardWorld) -> Vec<Placed> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(
        bevy_ecs::entity::Entity,
        &RefDes,
        &FootprintRef,
        &Position,
        &Rotation,
        Option<&Side>,
    )>();
    query
        .iter(ecs)
        .map(
            |(entity, refdes, footprint, position, rotation, side)| Placed {
                entity,
                refdes: refdes.as_str().to_string(),
                footprint: footprint.as_str().to_string(),
                position: position.0,
                rotation_deg: rotation.to_degrees(),
                side: side.copied(),
            },
        )
        .collect()
}

/// The four sides of a placed component's courtyard, in board coordinates.
///
/// Returns nothing for a footprint whose courtyard has no area, which is how
/// the library represents "not known" - there is no artwork to check.
/// The artwork a placed footprint prints, as segments in board coordinates.
///
/// A footprint that carries its own artwork prints that; one that does not
/// prints the courtyard outline the exporter derives, which is what
/// `gerber::silk` emits. A circle is walked as a polygon fine enough that the
/// error is under a tenth of its stroke - close enough to measure clearance
/// against, and far cheaper than a curve intersection.
fn silk_segments(footprint: &Footprint, placed: &Placed) -> Vec<(Point, Point)> {
    if footprint.silk.is_empty() {
        return courtyard_edges(footprint, placed);
    }

    let place = |p: Point| -> Point {
        let rotated = rotate_point(p, placed.rotation_deg);
        Point::new(
            Nm(placed.position.x.raw() + rotated.x.raw()),
            Nm(placed.position.y.raw() + rotated.y.raw()),
        )
    };

    let mut out = Vec::new();
    for shape in &footprint.silk {
        match shape {
            SilkShape::Segment { start, end, .. } => out.push((place(*start), place(*end))),
            SilkShape::Circle { centre, radius, .. } => {
                const STEPS: usize = 24;
                let radius = radius.raw() as f64;
                let mut previous = None;
                for step in 0..=STEPS {
                    let angle = step as f64 / STEPS as f64 * std::f64::consts::TAU;
                    let point = place(Point::new(
                        Nm(centre.x.raw() + (radius * angle.cos()).round() as i64),
                        Nm(centre.y.raw() + (radius * angle.sin()).round() as i64),
                    ));
                    if let Some(previous) = previous {
                        out.push((previous, point));
                    }
                    previous = Some(point);
                }
            }
        }
    }
    out
}

/// The strokes a part's printed name lays down, in board coordinates.
///
/// The same call the exporter makes, at the same default height, so what this
/// rule measures is what the legend file draws. A default that drifts from
/// `SilkConfig` here means the checker passes a board the fabricator will not,
/// which is the same hazard `SILK_LINE_WIDTH` is written down for.
fn designator_edges(footprint: &Footprint, placed: &Placed) -> Vec<(Point, Point)> {
    cypcb_world::silk_text::designator_strokes(
        &placed.refdes,
        placed.position,
        DESIGNATOR_HEIGHT,
        SILK_LINE_WIDTH,
        cypcb_world::silk_text::artwork_rise(footprint, placed.rotation_deg),
    )
    .into_iter()
    .filter_map(|shape| match shape {
        SilkShape::Segment { start, end, .. } => Some((start, end)),
        SilkShape::Circle { .. } => None,
    })
    .collect()
}

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

    /// A house that wants far more clearance than the exporter clipped for.
    ///
    /// The rule measures the legend as it will be printed - clipped off the
    /// copper at `EXPORT_CLEARANCE` - so at that clearance there is nothing
    /// left to report. Check the same board against a stricter house and the
    /// ink that was fine no longer is, which is what keeps this rule from
    /// being a formality.
    fn a_stricter_house() -> DesignRules {
        DesignRules {
            min_clearance: Nm::from_mm(0.5),
            ..DesignRules::jlcpcb_2layer()
        }
    }

    #[test]
    fn an_outline_landing_on_a_neighbours_pad_is_clipped_and_then_measured() {
        // Ink sits on the courtyard outline, not inside it, so the spacing that
        // matters is the one that walks a neighbour's pad onto that line.
        // Computed from the library rather than guessed.
        let library = FootprintLibrary::new();
        let smd = library.get("0402").expect("built-in");
        let half_court = smd.courtyard.max.x.raw() as f64 / 1_000_000.0;
        let half_span = smd.pads[0].position.x.raw().abs() as f64 / 1_000_000.0;

        let mut world = board_with_two_parts(half_court + half_span);
        assert!(
            SilkClearanceRule
                .check(&mut world, &DesignRules::jlcpcb_2layer())
                .is_empty(),
            "the exporter clips this ink off the pad, so there is nothing to report"
        );

        let violations = SilkClearanceRule.check(&mut world, &a_stricter_house());
        assert!(
            !violations.is_empty(),
            "a legend clipped for one house does not meet a stricter one"
        );
        assert_eq!(violations[0].kind, crate::ViolationKind::SilkClearance);
        assert!(
            violations[0].message.contains("silkscreen over R"),
            "the message names the part whose ink it is: {}",
            violations[0].message
        );
    }

    #[test]
    fn real_artwork_is_measured_instead_of_the_courtyard() {
        use cypcb_world::footprint::{FootprintLibrary, SilkShape};

        // A part whose legend is a single line reaching well past its own
        // courtyard - the kind of outline a supplier's footprint carries.
        let mut world = BoardWorld::new();
        world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

        let mut library = FootprintLibrary::new();
        let mut marked = library.get("0402").expect("built-in").clone();
        marked.name = "0402-MARKED".to_string();
        marked.silk = vec![SilkShape::Segment {
            start: Point::from_mm(0.0, 0.0),
            end: Point::from_mm(4.0, 0.0),
            width: Nm::from_mm(0.15),
        }];
        library.register(marked);
        world.set_footprints(library);

        world.spawn_component(
            RefDes::new("U1"),
            Value::new("part"),
            Position::from_mm(5.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402-MARKED"),
            NetConnections::new(),
        );
        // Its pad sits under where that line runs.
        world.spawn_component(
            RefDes::new("R9"),
            Value::new("10k"),
            Position::from_mm(9.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let violations = SilkClearanceRule.check(&mut world, &a_stricter_house());
        assert!(
            violations
                .iter()
                .any(|v| v.message.contains("U1 silkscreen over R9")),
            "the line reaches R9's copper, and only the artwork says so: {violations:?}"
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
