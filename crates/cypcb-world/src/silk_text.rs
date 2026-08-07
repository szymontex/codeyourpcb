//! A stroke font, so a legend can carry the names of the parts it labels.
//!
//! A fabricated board with no `R1` printed beside R1 cannot be assembled by
//! eye: the person holding the reel has to read the design file instead of the
//! board. Gerber has no text primitive that a fabricator is obliged to honour,
//! so a legend's letters are drawn the way its outlines are - as strokes.
//!
//! Glyphs live on a 0..1 box and are described as polylines. Only what a
//! reference designator needs is here: capitals, digits, and the three
//! separators that turn up in net and part names. Anything else prints as
//! nothing rather than as a wrong character.
//!
//! # Why this lives in the model rather than in the exporter
//!
//! The letters started life inside `cypcb-export`, which is the only crate
//! that draws them - and that made the silkscreen clearance rule blind to
//! them. Ink printed over a pad starves the joint under it, so the checker
//! exists to catch exactly this, and it was measuring courtyard outlines while
//! the exporter printed designators the checker had never heard of. Both read
//! [`designator_strokes`] now, so what is checked is what is printed.

use cypcb_core::{Nm, Point};

use crate::footprint::{Footprint, SilkShape};

/// A glyph as polylines over a unit box: x to the right, y up, both 0.0..1.0.
type Strokes = &'static [&'static [(f32, f32)]];

/// A square of board a legend must not print inside.
///
/// One per pad, centred on the pad and large enough that anything outside it
/// is at least the required clearance from the pad's copper - see
/// [`pad_keepouts`].
#[derive(Debug, Clone, Copy)]
pub struct Keepout {
    /// Where the pad is, in board coordinates.
    pub centre: Point,
    /// Half the square's side.
    pub half_size: Nm,
}

impl Keepout {
    fn holds(&self, point: Point) -> bool {
        (point.x.raw() - self.centre.x.raw()).abs() <= self.half_size.raw()
            && (point.y.raw() - self.centre.y.raw()).abs() <= self.half_size.raw()
    }
}

/// The squares a legend must stay out of, one per pad on the given face.
///
/// A board house clips silkscreen off solderable copper before it prints;
/// shipping a legend that has to be clipped means shipping a file whose
/// legend nobody has seen. The exporter does it here instead, so the Gerber
/// that leaves is the Gerber that gets made.
///
/// The square is centred on the pad, with a half-side of half the pad's
/// longer dimension plus `margin`. That is deliberately larger than the pad:
/// anything outside a square of half-side `s` is further than `s` from its
/// centre in the plane, so clipping to it satisfies a checker that treats the
/// pad as a disc.
///
/// Pass `margin` as the fabricator's silk-to-copper clearance plus half the
/// stroke width, because ink spreads half a stroke either side of its
/// centreline.
///
/// A pad rotated by anything other than a right angle is enclosed by its
/// bounding square, which is the conservative direction.
pub fn pad_keepouts(
    world: &mut crate::BoardWorld,
    library: &crate::footprint::FootprintLibrary,
    layer: crate::Layer,
    margin: Nm,
) -> Vec<Keepout> {
    use crate::components::{FootprintRef, Position, Rotation};

    let placed: Vec<(Point, String, f64)> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(&Position, &FootprintRef, &Rotation)>();
        query
            .iter(ecs)
            .map(|(position, footprint, rotation)| {
                (position.0, footprint.0.clone(), rotation.to_degrees())
            })
            .collect()
    };

    let mut keepouts = Vec::new();
    for (position, footprint_name, rotation_deg) in placed {
        let Some(footprint) = library.get(&footprint_name) else {
            continue;
        };
        let (sin, cos) = rotation_deg.to_radians().sin_cos();

        for pad in &footprint.pads {
            if !pad.layers.contains(&layer) {
                continue;
            }
            let x = pad.position.x.raw() as f64;
            let y = pad.position.y.raw() as f64;
            let centre = Point::new(
                Nm(position.x.raw() + (x * cos - y * sin).round() as i64),
                Nm(position.y.raw() + (x * sin + y * cos).round() as i64),
            );
            let half_size = Nm(pad.size.0.raw().max(pad.size.1.raw()) / 2 + margin.raw());
            keepouts.push(Keepout { centre, half_size });
        }
    }

    keepouts
}

/// Cut every stroke where it enters a keepout, keeping the parts outside.
///
/// A letter crossing a pad comes back as the pieces of itself that print, or
/// as nothing at all when the pad swallows it. Circles are left whole: nothing
/// in the built-in library draws one over copper, and a clipped arc is a
/// different shape rather than a shorter one.
pub fn clip_strokes(strokes: Vec<SilkShape>, keepouts: &[Keepout]) -> Vec<SilkShape> {
    if keepouts.is_empty() {
        return strokes;
    }

    let mut out = Vec::with_capacity(strokes.len());
    for shape in strokes {
        let SilkShape::Segment { start, end, width } = shape else {
            out.push(shape);
            continue;
        };

        // Each keepout cuts the pieces the previous ones left.
        let mut pieces = vec![(start, end)];
        for keepout in keepouts {
            let mut next = Vec::with_capacity(pieces.len());
            for (from, to) in pieces {
                next.extend(cut(from, to, keepout));
            }
            pieces = next;
            if pieces.is_empty() {
                break;
            }
        }

        for (from, to) in pieces {
            if from != to {
                out.push(SilkShape::Segment {
                    start: from,
                    end: to,
                    width,
                });
            }
        }
    }

    out
}

/// The parts of a segment that lie outside one keepout.
///
/// Liang-Barsky against an axis-aligned square: find the interval of the
/// segment that is inside, then hand back what is on either side of it.
fn cut(start: Point, end: Point, keepout: &Keepout) -> Vec<(Point, Point)> {
    let dx = (end.x.raw() - start.x.raw()) as f64;
    let dy = (end.y.raw() - start.y.raw()) as f64;

    if dx == 0.0 && dy == 0.0 {
        return if keepout.holds(start) {
            Vec::new()
        } else {
            vec![(start, end)]
        };
    }

    let half = keepout.half_size.raw() as f64;
    let min_x = keepout.centre.x.raw() as f64 - half;
    let max_x = keepout.centre.x.raw() as f64 + half;
    let min_y = keepout.centre.y.raw() as f64 - half;
    let max_y = keepout.centre.y.raw() as f64 + half;

    let mut enter: f64 = 0.0;
    let mut leave: f64 = 1.0;
    let x0 = start.x.raw() as f64;
    let y0 = start.y.raw() as f64;

    for (delta, from, low, high) in [(dx, x0, min_x, max_x), (dy, y0, min_y, max_y)] {
        if delta == 0.0 {
            if from < low || from > high {
                // Parallel to this pair of edges and outside them: the segment
                // never enters the square.
                return vec![(start, end)];
            }
            continue;
        }
        let (near, far) = {
            let a = (low - from) / delta;
            let b = (high - from) / delta;
            if a <= b {
                (a, b)
            } else {
                (b, a)
            }
        };
        enter = enter.max(near);
        leave = leave.min(far);
    }

    if enter >= leave {
        return vec![(start, end)];
    }

    let at = |t: f64| -> Point {
        Point::new(
            Nm((x0 + dx * t).round() as i64),
            Nm((y0 + dy * t).round() as i64),
        )
    };

    let mut pieces = Vec::new();
    if enter > 0.0 {
        pieces.push((start, at(enter)));
    }
    if leave < 1.0 {
        pieces.push((at(leave), end));
    }
    pieces
}

/// The strokes for one character, or `None` if this font cannot draw it.
///
/// One glyph per line, on purpose. This is a drawing held as a table, and the
/// only way to check a letter by eye is to read its points in order across one
/// line - rustfmt spreads each coordinate onto its own line and turns a
/// hundred-line font into four hundred lines nobody can proofread.
#[rustfmt::skip]
pub fn glyph(c: char) -> Option<Strokes> {
    let upper = c.to_ascii_uppercase();
    Some(match upper {
        'A' => &[&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], &[(0.2, 0.4), (0.8, 0.4)]],
        'B' => &[
            &[(0.0, 0.0), (0.0, 1.0), (0.7, 1.0), (0.9, 0.8), (0.7, 0.5), (0.0, 0.5)],
            &[(0.7, 0.5), (0.95, 0.25), (0.7, 0.0), (0.0, 0.0)],
        ],
        'C' => &[&[(1.0, 0.85), (0.7, 1.0), (0.2, 1.0), (0.0, 0.7), (0.0, 0.3), (0.2, 0.0), (0.7, 0.0), (1.0, 0.15)]],
        'D' => &[&[(0.0, 0.0), (0.0, 1.0), (0.6, 1.0), (1.0, 0.7), (1.0, 0.3), (0.6, 0.0), (0.0, 0.0)]],
        'E' => &[&[(1.0, 1.0), (0.0, 1.0), (0.0, 0.0), (1.0, 0.0)], &[(0.0, 0.5), (0.7, 0.5)]],
        'F' => &[&[(1.0, 1.0), (0.0, 1.0), (0.0, 0.0)], &[(0.0, 0.5), (0.7, 0.5)]],
        'G' => &[&[(1.0, 0.85), (0.7, 1.0), (0.2, 1.0), (0.0, 0.7), (0.0, 0.3), (0.2, 0.0), (0.7, 0.0), (1.0, 0.2), (1.0, 0.45), (0.5, 0.45)]],
        'H' => &[&[(0.0, 1.0), (0.0, 0.0)], &[(1.0, 1.0), (1.0, 0.0)], &[(0.0, 0.5), (1.0, 0.5)]],
        'I' => &[&[(0.5, 1.0), (0.5, 0.0)], &[(0.2, 1.0), (0.8, 1.0)], &[(0.2, 0.0), (0.8, 0.0)]],
        'J' => &[&[(1.0, 1.0), (1.0, 0.25), (0.8, 0.0), (0.3, 0.0), (0.0, 0.25)]],
        'K' => &[&[(0.0, 1.0), (0.0, 0.0)], &[(1.0, 1.0), (0.0, 0.45)], &[(0.25, 0.6), (1.0, 0.0)]],
        'L' => &[&[(0.0, 1.0), (0.0, 0.0), (1.0, 0.0)]],
        'M' => &[&[(0.0, 0.0), (0.0, 1.0), (0.5, 0.5), (1.0, 1.0), (1.0, 0.0)]],
        'N' => &[&[(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)]],
        'O' => &[&[(0.2, 1.0), (0.8, 1.0), (1.0, 0.7), (1.0, 0.3), (0.8, 0.0), (0.2, 0.0), (0.0, 0.3), (0.0, 0.7), (0.2, 1.0)]],
        'P' => &[&[(0.0, 0.0), (0.0, 1.0), (0.7, 1.0), (1.0, 0.75), (0.7, 0.5), (0.0, 0.5)]],
        'Q' => &[
            &[(0.2, 1.0), (0.8, 1.0), (1.0, 0.7), (1.0, 0.3), (0.8, 0.0), (0.2, 0.0), (0.0, 0.3), (0.0, 0.7), (0.2, 1.0)],
            &[(0.6, 0.3), (1.0, -0.1)],
        ],
        'R' => &[
            &[(0.0, 0.0), (0.0, 1.0), (0.7, 1.0), (1.0, 0.75), (0.7, 0.5), (0.0, 0.5)],
            &[(0.5, 0.5), (1.0, 0.0)],
        ],
        'S' => &[&[(1.0, 0.85), (0.7, 1.0), (0.2, 1.0), (0.0, 0.8), (0.2, 0.55), (0.8, 0.45), (1.0, 0.2), (0.8, 0.0), (0.3, 0.0), (0.0, 0.15)]],
        'T' => &[&[(0.0, 1.0), (1.0, 1.0)], &[(0.5, 1.0), (0.5, 0.0)]],
        'U' => &[&[(0.0, 1.0), (0.0, 0.25), (0.25, 0.0), (0.75, 0.0), (1.0, 0.25), (1.0, 1.0)]],
        'V' => &[&[(0.0, 1.0), (0.5, 0.0), (1.0, 1.0)]],
        'W' => &[&[(0.0, 1.0), (0.25, 0.0), (0.5, 0.6), (0.75, 0.0), (1.0, 1.0)]],
        'X' => &[&[(0.0, 1.0), (1.0, 0.0)], &[(0.0, 0.0), (1.0, 1.0)]],
        'Y' => &[&[(0.0, 1.0), (0.5, 0.5), (1.0, 1.0)], &[(0.5, 0.5), (0.5, 0.0)]],
        'Z' => &[&[(0.0, 1.0), (1.0, 1.0), (0.0, 0.0), (1.0, 0.0)]],
        '0' => &[
            &[(0.2, 1.0), (0.8, 1.0), (1.0, 0.7), (1.0, 0.3), (0.8, 0.0), (0.2, 0.0), (0.0, 0.3), (0.0, 0.7), (0.2, 1.0)],
            &[(0.0, 0.0), (1.0, 1.0)],
        ],
        '1' => &[&[(0.2, 0.8), (0.5, 1.0), (0.5, 0.0)], &[(0.2, 0.0), (0.8, 0.0)]],
        '2' => &[&[(0.0, 0.8), (0.25, 1.0), (0.75, 1.0), (1.0, 0.75), (0.0, 0.0), (1.0, 0.0)]],
        '3' => &[&[(0.0, 1.0), (1.0, 1.0), (0.45, 0.55)], &[(0.45, 0.55), (1.0, 0.3), (0.75, 0.0), (0.2, 0.0), (0.0, 0.15)]],
        '4' => &[&[(0.75, 0.0), (0.75, 1.0), (0.0, 0.3), (1.0, 0.3)]],
        '5' => &[&[(1.0, 1.0), (0.0, 1.0), (0.0, 0.55), (0.7, 0.6), (1.0, 0.35), (0.8, 0.0), (0.2, 0.0), (0.0, 0.15)]],
        '6' => &[&[(1.0, 0.85), (0.6, 1.0), (0.2, 0.8), (0.0, 0.35), (0.2, 0.0), (0.7, 0.0), (1.0, 0.25), (0.8, 0.55), (0.2, 0.6), (0.0, 0.35)]],
        '7' => &[&[(0.0, 1.0), (1.0, 1.0), (0.35, 0.0)]],
        '8' => &[
            &[(0.3, 0.55), (0.05, 0.75), (0.25, 1.0), (0.75, 1.0), (0.95, 0.75), (0.7, 0.55), (0.3, 0.55)],
            &[(0.3, 0.55), (0.0, 0.3), (0.25, 0.0), (0.75, 0.0), (1.0, 0.3), (0.7, 0.55)],
        ],
        '9' => &[&[(0.0, 0.15), (0.35, 0.0), (0.85, 0.2), (1.0, 0.65), (0.8, 1.0), (0.3, 1.0), (0.0, 0.75), (0.25, 0.45), (0.85, 0.4), (1.0, 0.65)]],
        '-' => &[&[(0.1, 0.5), (0.9, 0.5)]],
        '_' => &[&[(0.0, 0.0), (1.0, 0.0)]],
        '.' => &[&[(0.4, 0.0), (0.6, 0.0)]],
        ' ' => &[],
        _ => return None,
    })
}

/// How many glyph widths a string occupies, spacing included.
///
/// Characters this font cannot draw still take their place, so a name with one
/// unknown character stays aligned rather than shuffling left.
pub fn width_in_glyphs(text: &str) -> f32 {
    let count = text.chars().count();
    if count == 0 {
        0.0
    } else {
        count as f32 * 1.0 + (count - 1) as f32 * ADVANCE_GAP
    }
}

/// Space between glyph boxes, as a fraction of a glyph's width.
pub const ADVANCE_GAP: f32 = 0.35;

/// How wide a glyph is, as a fraction of how tall it is.
pub const GLYPH_ASPECT: f32 = 0.6;

/// The least a designator rises above the part's origin, as a fraction of the
/// text height.
///
/// A floor, not the answer. A part whose own artwork reaches higher than this
/// pushes its name further up - see [`artwork_rise`].
pub const BASELINE_RISE: f32 = 0.6;

/// How far above a footprint's own artwork a designator sits, in stroke
/// widths.
///
/// The artwork - a courtyard or the footprint's own lines - already encloses
/// the part's copper, so clearing it clears the pads. Two strokes leaves
/// roughly 0.23mm between the edge of the ink and the courtyard at the
/// project's defaults, comfortably past a fabricator's 0.13mm. Comfortably is
/// not proof: `silk-clearance` measures what this actually produced.
pub const ARTWORK_GAP_STROKES: f32 = 2.0;

/// How high a placed footprint's own artwork reaches above its origin.
///
/// The courtyard when the footprint has no artwork of its own, its lines and
/// circles when it does - the same choice `gerber::silk` makes when deciding
/// what to draw. Rotation is applied, because a part turned on its side is
/// taller than the same part lying flat.
///
/// Zero for a footprint that declares neither, which is how the library says
/// "not known".
pub fn artwork_rise(footprint: &Footprint, rotation_deg: f64) -> Nm {
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let rotated_y = |point: Point| -> i64 {
        (point.x.raw() as f64 * sin + point.y.raw() as f64 * cos).round() as i64
    };

    let highest = if footprint.silk.is_empty() {
        let court = &footprint.courtyard;
        if court.min.x.raw() >= court.max.x.raw() || court.min.y.raw() >= court.max.y.raw() {
            return Nm(0);
        }
        [
            Point::new(court.min.x, court.min.y),
            Point::new(court.max.x, court.min.y),
            Point::new(court.max.x, court.max.y),
            Point::new(court.min.x, court.max.y),
        ]
        .into_iter()
        .map(rotated_y)
        .max()
    } else {
        footprint
            .silk
            .iter()
            .flat_map(|shape| match shape {
                SilkShape::Segment { start, end, width } => {
                    let half = width.raw() / 2;
                    vec![rotated_y(*start) + half, rotated_y(*end) + half]
                }
                SilkShape::Circle {
                    centre,
                    radius,
                    width,
                } => vec![rotated_y(*centre) + radius.raw() + width.raw() / 2],
            })
            .max()
    };

    Nm(highest.unwrap_or(0).max(0))
}

/// The strokes a part's designator prints, in board coordinates.
///
/// One [`SilkShape::Segment`] per drawn line, at the given stroke width, laid
/// out centred on `centre.x` and sitting above both the part's origin and its
/// own artwork - pass [`artwork_rise`] as `rise`. Characters this font cannot
/// draw take up their space and print nothing, so a name with one odd
/// character stays aligned.
///
/// Empty for an empty name, and empty when no character in the name has a
/// glyph - which is what tells the exporter to fall back to a position mark.
///
/// The text is not rotated with the part. A designator is read by a person
/// holding the board, and turning it with the part it labels would print half
/// the names upside down.
pub fn designator_strokes(
    text: &str,
    centre: Point,
    height: Nm,
    width: Nm,
    rise: Nm,
) -> Vec<SilkShape> {
    let height_nm = height.raw() as f32;
    let glyph_width = height_nm * GLYPH_ASPECT;
    let advance = glyph_width * (1.0 + ADVANCE_GAP);
    let total = width_in_glyphs(text) * glyph_width;
    let start_x = centre.x.raw() as f32 - total / 2.0;
    // Above the part's own artwork, or the floor, whichever is higher. A name
    // printed inside the courtyard is a name printed on the pads the courtyard
    // encloses.
    let clear_of_artwork = rise.raw() as f32 + width.raw() as f32 * ARTWORK_GAP_STROKES;
    let baseline_y = centre.y.raw() as f32 + clear_of_artwork.max(height_nm * BASELINE_RISE);

    let mut out = Vec::new();
    for (index, character) in text.chars().enumerate() {
        let Some(strokes) = glyph(character) else {
            continue;
        };
        let origin_x = start_x + index as f32 * advance;

        for stroke in strokes {
            let point_at = |(gx, gy): &(f32, f32)| {
                Point::new(
                    Nm((origin_x + gx * glyph_width) as i64),
                    Nm((baseline_y + gy * height_nm) as i64),
                )
            };
            for pair in stroke.windows(2) {
                out.push(SilkShape::Segment {
                    start: point_at(&pair[0]),
                    end: point_at(&pair[1]),
                    width,
                });
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_character_a_designator_can_hold_has_a_glyph() {
        // A refdes is letters and digits, and net names bring the separators.
        // A missing glyph prints nothing, which is worse than an ugly one: the
        // board says `R` where the design says `R1`.
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_. ".chars() {
            assert!(glyph(c).is_some(), "no glyph for {c:?}");
        }
    }

    #[test]
    fn a_character_outside_the_font_draws_nothing_rather_than_something_wrong() {
        assert!(glyph('#').is_none());
        assert!(glyph('%').is_none());
    }

    /// The box the name occupies, as the strokes actually land.
    fn extent(shapes: &[SilkShape]) -> (i64, i64, i64, i64) {
        let points = shapes.iter().flat_map(|shape| match shape {
            SilkShape::Segment { start, end, .. } => [*start, *end],
            SilkShape::Circle { centre, .. } => [*centre, *centre],
        });
        points.fold(
            (i64::MAX, i64::MIN, i64::MAX, i64::MIN),
            |(min_x, max_x, min_y, max_y), point| {
                (
                    min_x.min(point.x.raw()),
                    max_x.max(point.x.raw()),
                    min_y.min(point.y.raw()),
                    max_y.max(point.y.raw()),
                )
            },
        )
    }

    #[test]
    fn a_name_sits_centred_over_the_part_and_above_it() {
        let centre = Point::new(Nm::from_mm(10.0), Nm::from_mm(20.0));
        let height = Nm::from_mm(1.0);
        let strokes = designator_strokes("R1", centre, height, Nm::from_mm(0.15), Nm(0));
        assert!(!strokes.is_empty());

        let (min_x, max_x, min_y, max_y) = extent(&strokes);

        // Centred on the part: the two margins agree to within a stroke.
        let left = centre.x.raw() - min_x;
        let right = max_x - centre.x.raw();
        assert!(
            (left - right).abs() < Nm::from_mm(0.15).raw(),
            "a name has to sit centred on the part it labels, margins {left} and {right}"
        );

        // Above it: nothing dips to the part's own origin.
        assert!(
            min_y > centre.y.raw(),
            "the name has to clear the part's centre, lowest stroke at {min_y}"
        );
        assert!(
            max_y - min_y <= height.raw(),
            "a one-line name cannot be taller than its text height"
        );
    }

    #[test]
    fn a_name_with_no_glyphs_draws_nothing_so_the_caller_can_fall_back() {
        let centre = Point::new(Nm::from_mm(1.0), Nm::from_mm(1.0));
        let nothing = designator_strokes("##", centre, Nm::from_mm(1.0), Nm::from_mm(0.15), Nm(0));
        assert!(nothing.is_empty());
        assert!(
            designator_strokes("", centre, Nm::from_mm(1.0), Nm::from_mm(0.15), Nm(0)).is_empty()
        );
    }

    #[test]
    fn a_taller_name_takes_more_room_in_both_axes() {
        // Height is the only dial the exporter exposes, and it has to move the
        // artwork rather than only the glyph boxes.
        let centre = Point::new(Nm::from_mm(5.0), Nm::from_mm(5.0));
        let small = extent(&designator_strokes(
            "C12",
            centre,
            Nm::from_mm(1.0),
            Nm::from_mm(0.15),
            Nm(0),
        ));
        let large = extent(&designator_strokes(
            "C12",
            centre,
            Nm::from_mm(2.0),
            Nm::from_mm(0.15),
            Nm(0),
        ));
        assert!(large.1 - large.0 > small.1 - small.0, "wider");
        assert!(large.3 - large.2 > small.3 - small.2, "taller");
    }

    #[test]
    fn strokes_stay_inside_the_box_they_are_measured_against() {
        // Placement assumes a glyph fits 0..1 in both axes. The tail of a Q is
        // the one deliberate exception and it hangs below the baseline.
        for c in "ABCDEFGHIJKLMNOPRSTUVWXYZ0123456789-_.".chars() {
            for stroke in glyph(c).expect("a glyph") {
                for (x, y) in *stroke {
                    assert!(
                        (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y),
                        "{c:?} has a point at ({x}, {y}) outside its box"
                    );
                }
            }
        }
    }
}
