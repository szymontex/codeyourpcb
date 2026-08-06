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
