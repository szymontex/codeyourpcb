//! The shape a copper pour actually takes.
//!
//! A zone is a rectangle the designer draws and a promise that everything
//! inside it becomes copper on one net. What is made is that rectangle minus
//! every other piece of copper on the layer, each grown by the clearance the
//! fab requires - otherwise the pour shorts the board it was meant to ground.
//!
//! This is the geometry half. Both the zone and the obstacles are
//! axis-aligned, so subtracting one from the other splits it into at most four
//! rectangles and the result stays a set of rectangles: exact, cheap, and
//! testable by area. Curved and rotated copper is boxed first, which pulls the
//! pour back further than it strictly needs - the safe direction, and the same
//! one the checker takes elsewhere.

use cypcb_core::{Nm, Point, Rect};

/// Subtract `obstacles` from `zone`, returning the copper that remains.
///
/// Each obstacle is grown by `clearance` before it is cut out. The result is a
/// set of disjoint rectangles covering exactly the part of the zone no
/// obstacle claims; an empty result means the zone is entirely consumed.
pub fn fill(zone: Rect, obstacles: &[Rect], clearance: Nm) -> Vec<Rect> {
    let mut pieces = vec![zone];

    for obstacle in obstacles {
        let keepout = grow(*obstacle, clearance);
        let mut next = Vec::with_capacity(pieces.len() + 3);
        for piece in pieces {
            next.extend(subtract(piece, keepout));
        }
        pieces = next;
        if pieces.is_empty() {
            break;
        }
    }

    pieces
}

/// Grow a rectangle by `margin` on every side.
pub fn grown(rect: Rect, margin: Nm) -> Rect {
    grow(rect, margin)
}

/// Grow a rectangle by `margin` on every side.
fn grow(rect: Rect, margin: Nm) -> Rect {
    Rect {
        min: Point::new(Nm(rect.min.x.0 - margin.0), Nm(rect.min.y.0 - margin.0)),
        max: Point::new(Nm(rect.max.x.0 + margin.0), Nm(rect.max.y.0 + margin.0)),
    }
}

/// Cut `hole` out of `piece`, returning what is left as up to four rectangles.
///
/// The pieces are taken in bands - below the hole, above it, then the left and
/// right of what remains beside it - so they never overlap each other.
fn subtract(piece: Rect, hole: Rect) -> Vec<Rect> {
    let overlaps = piece.min.x.0 < hole.max.x.0
        && hole.min.x.0 < piece.max.x.0
        && piece.min.y.0 < hole.max.y.0
        && hole.min.y.0 < piece.max.y.0;
    if !overlaps {
        return vec![piece];
    }

    let mut out = Vec::with_capacity(4);

    // Below the hole.
    if piece.min.y.0 < hole.min.y.0 {
        out.push(Rect {
            min: piece.min,
            max: Point::new(piece.max.x, hole.min.y),
        });
    }
    // Above it.
    if hole.max.y.0 < piece.max.y.0 {
        out.push(Rect {
            min: Point::new(piece.min.x, hole.max.y),
            max: piece.max,
        });
    }

    // The band beside the hole, split left and right.
    let band_bottom = piece.min.y.0.max(hole.min.y.0);
    let band_top = piece.max.y.0.min(hole.max.y.0);
    if band_bottom < band_top {
        if piece.min.x.0 < hole.min.x.0 {
            out.push(Rect {
                min: Point::new(piece.min.x, Nm(band_bottom)),
                max: Point::new(hole.min.x, Nm(band_top)),
            });
        }
        if hole.max.x.0 < piece.max.x.0 {
            out.push(Rect {
                min: Point::new(hole.max.x, Nm(band_bottom)),
                max: Point::new(piece.max.x, Nm(band_top)),
            });
        }
    }

    out
}

/// What two rectangles have in common, or `None` when they miss each other.
pub fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let min_x = a.min.x.0.max(b.min.x.0);
    let min_y = a.min.y.0.max(b.min.y.0);
    let max_x = a.max.x.0.min(b.max.x.0);
    let max_y = a.max.y.0.min(b.max.y.0);
    if min_x >= max_x || min_y >= max_y {
        return None;
    }
    Some(Rect {
        min: Point::new(Nm(min_x), Nm(min_y)),
        max: Point::new(Nm(max_x), Nm(max_y)),
    })
}

/// The area a rectangle covers, in square nanometres.
pub fn area(rect: Rect) -> i128 {
    let w = (rect.max.x.0 - rect.min.x.0).max(0) as i128;
    let h = (rect.max.y.0 - rect.min.y.0).max(0) as i128;
    w * h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> Rect {
        Rect {
            min: Point::from_mm(x1, y1),
            max: Point::from_mm(x2, y2),
        }
    }

    #[test]
    fn a_zone_with_nothing_in_it_is_itself() {
        let zone = rect(0.0, 0.0, 10.0, 10.0);
        let filled = fill(zone, &[], Nm::ZERO);
        assert_eq!(filled.len(), 1);
        assert_eq!(area(filled[0]), area(zone));
    }

    #[test]
    fn a_hole_in_the_middle_leaves_four_pieces_and_the_right_area() {
        // A 10x10 zone with a 2x2 obstacle at its centre, no clearance: the
        // copper left is 100 - 4 square millimetres, and it comes out in four
        // pieces because the hole touches no edge.
        let zone = rect(0.0, 0.0, 10.0, 10.0);
        let obstacle = rect(4.0, 4.0, 6.0, 6.0);

        let filled = fill(zone, &[obstacle], Nm::ZERO);
        assert_eq!(filled.len(), 4, "a hole in the middle splits the zone");

        let covered: i128 = filled.iter().map(|r| area(*r)).sum();
        assert_eq!(
            covered,
            area(zone) - area(obstacle),
            "the copper left is the zone minus the hole"
        );
    }

    #[test]
    fn clearance_makes_the_hole_bigger_than_the_obstacle() {
        let zone = rect(0.0, 0.0, 10.0, 10.0);
        let obstacle = rect(4.0, 4.0, 6.0, 6.0);

        let tight = fill(zone, &[obstacle], Nm::ZERO);
        let spaced = fill(zone, &[obstacle], Nm::from_mm(0.5));

        let tight_area: i128 = tight.iter().map(|r| area(*r)).sum();
        let spaced_area: i128 = spaced.iter().map(|r| area(*r)).sum();
        assert!(
            spaced_area < tight_area,
            "clearance has to pull the copper back: {spaced_area} against {tight_area}"
        );

        // 3x3 of hole instead of 2x2, so exactly 5 square millimetres more is
        // gone.
        let square_mm = 1_000_000i128 * 1_000_000;
        assert_eq!(tight_area - spaced_area, 5 * square_mm);
    }

    #[test]
    fn an_obstacle_outside_the_zone_changes_nothing() {
        let zone = rect(0.0, 0.0, 10.0, 10.0);
        let filled = fill(zone, &[rect(20.0, 20.0, 25.0, 25.0)], Nm::from_mm(0.5));
        assert_eq!(filled.len(), 1);
        assert_eq!(area(filled[0]), area(zone));
    }

    #[test]
    fn an_obstacle_that_swallows_the_zone_leaves_no_copper() {
        let zone = rect(0.0, 0.0, 10.0, 10.0);
        let filled = fill(zone, &[rect(-1.0, -1.0, 11.0, 11.0)], Nm::ZERO);
        assert!(
            filled.is_empty(),
            "a zone entirely under copper pours nothing: {filled:?}"
        );
    }

    #[test]
    fn the_pieces_never_overlap_each_other() {
        // Overlapping output would be drawn twice, which a fabricator reads as
        // one shape but a checker measuring copper area would double-count.
        let zone = rect(0.0, 0.0, 20.0, 20.0);
        let obstacles = [
            rect(2.0, 2.0, 5.0, 5.0),
            rect(8.0, 3.0, 12.0, 7.0),
            rect(14.0, 14.0, 18.0, 18.0),
            rect(4.0, 12.0, 7.0, 16.0),
        ];

        let filled = fill(zone, &obstacles, Nm::from_mm(0.2));
        for (i, a) in filled.iter().enumerate() {
            for b in filled.iter().skip(i + 1) {
                let overlap = a.min.x.0 < b.max.x.0
                    && b.min.x.0 < a.max.x.0
                    && a.min.y.0 < b.max.y.0
                    && b.min.y.0 < a.max.y.0;
                assert!(!overlap, "two pieces overlap: {a:?} and {b:?}");
            }
        }

        // And nothing left touches an obstacle's keepout.
        for piece in &filled {
            for obstacle in &obstacles {
                let keepout = grow(*obstacle, Nm::from_mm(0.2));
                let overlap = piece.min.x.0 < keepout.max.x.0
                    && keepout.min.x.0 < piece.max.x.0
                    && piece.min.y.0 < keepout.max.y.0
                    && keepout.min.y.0 < piece.max.y.0;
                assert!(!overlap, "{piece:?} reaches into {keepout:?}");
            }
        }
    }
}

/// How a pour is drawn: what it keeps away from, and how it meets its own net.
#[derive(Debug, Clone, Copy)]
pub struct PourOptions {
    /// Distance kept from copper on another net.
    pub clearance: Nm,
    /// Gap cut around a pad on the pour's own net, before the spokes go back.
    pub thermal_gap: Nm,
    /// Width of each spoke bridging that gap.
    pub spoke_width: Nm,
}

impl Default for PourOptions {
    fn default() -> Self {
        // Generous where a wrong guess is only a smaller pour, and matching
        // the fab presets where a wrong guess would be a bad joint.
        PourOptions {
            clearance: Nm::from_mm(0.3),
            thermal_gap: Nm::from_mm(0.254),
            spoke_width: Nm::from_mm(0.254),
        }
    }
}

/// The copper that bridges a thermal gap: two crossing bars, four spokes.
///
/// A pad on the pour's own net is cut out with `thermal_gap` around it and
/// then reconnected by these, rather than flooded solid. Solid copper conducts
/// heat away from the joint faster than an iron can put it in, which is why a
/// hand-soldered ground pin on a plane lifts pads instead of wetting.
///
/// Each bar spans the pad's keepout exactly, so it fills the ring and no more;
/// the pad itself is copper already.
pub fn thermal_spokes(pad: Rect, options: &PourOptions) -> Vec<Rect> {
    let keepout = grow(pad, options.thermal_gap);
    let half = options.spoke_width.0 / 2;
    let cx = (pad.min.x.0 + pad.max.x.0) / 2;
    let cy = (pad.min.y.0 + pad.max.y.0) / 2;

    vec![
        // Across.
        Rect {
            min: Point::new(keepout.min.x, Nm(cy - half)),
            max: Point::new(keepout.max.x, Nm(cy + half)),
        },
        // And up.
        Rect {
            min: Point::new(Nm(cx - half), keepout.min.y),
            max: Point::new(Nm(cx + half), keepout.max.y),
        },
    ]
}

#[cfg(test)]
mod thermal_tests {
    use super::*;

    fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> Rect {
        Rect {
            min: Point::from_mm(x1, y1),
            max: Point::from_mm(x2, y2),
        }
    }

    #[test]
    fn a_spoke_reaches_from_the_pad_to_the_pour() {
        // The bar has to span the whole keepout, or the gap it is supposed to
        // bridge stays open and the pad is not connected to the plane at all.
        let pad = rect(9.7, 9.75, 10.3, 10.25);
        let options = PourOptions::default();
        let spokes = thermal_spokes(pad, &options);

        let keepout = grow(pad, options.thermal_gap);
        assert_eq!(spokes.len(), 2, "two crossing bars make four spokes");
        assert_eq!(spokes[0].min.x, keepout.min.x);
        assert_eq!(spokes[0].max.x, keepout.max.x);
        assert_eq!(spokes[1].min.y, keepout.min.y);
        assert_eq!(spokes[1].max.y, keepout.max.y);
    }

    #[test]
    fn a_spoke_is_as_wide_as_the_rules_ask() {
        let pad = rect(9.7, 9.75, 10.3, 10.25);
        let options = PourOptions {
            spoke_width: Nm::from_mm(0.4),
            ..PourOptions::default()
        };
        let spokes = thermal_spokes(pad, &options);
        let height = spokes[0].max.y.0 - spokes[0].min.y.0;
        assert_eq!(height, Nm::from_mm(0.4).0, "the bar is the spoke width");
    }

    #[test]
    fn the_gap_is_the_only_copper_removed_around_a_pad_on_the_pour_s_net() {
        // Cut the keepout, put the spokes back: what is left missing is the
        // gap minus the spokes, which is what a thermal relief looks like.
        let zone = rect(0.0, 0.0, 20.0, 20.0);
        let pad = rect(9.7, 9.75, 10.3, 10.25);
        let options = PourOptions::default();

        let keepout = grow(pad, options.thermal_gap);
        let filled = fill(zone, &[grow(pad, options.thermal_gap)], Nm::ZERO);
        let copper: i128 = filled.iter().map(|r| area(*r)).sum();
        assert_eq!(copper, area(zone) - area(keepout));

        // Both bars sit inside the keepout, so adding them back can only put
        // copper into the hole that was just cut.
        for spoke in thermal_spokes(pad, &options) {
            assert!(
                spoke.min.x.0 >= keepout.min.x.0
                    && spoke.max.x.0 <= keepout.max.x.0
                    && spoke.min.y.0 >= keepout.min.y.0
                    && spoke.max.y.0 <= keepout.max.y.0,
                "a spoke leaves the keepout it is filling: {spoke:?}"
            );
        }
    }
}
