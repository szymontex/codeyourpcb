//! The outline of an area: a closed ring of points.
//!
//! A zone drawn as a polygon is a ring of corners, joined in order and closed
//! back to the first. Everything asked of it here is asked in whole
//! nanometres and answered exactly: the cross products are taken in `i128`, so
//! a point is on an edge or it is not, and no tolerance decides which.
//!
//! The ring may be concave and may name its first corner again at the end; the
//! functions read it either way. A ring of fewer than three corners holds
//! nothing.

use crate::{Nm, Point, Rect};

/// The ring without a closing corner that repeats the first.
fn open(points: &[Point]) -> &[Point] {
    match points {
        [first, .., last] if first == last => &points[..points.len() - 1],
        _ => points,
    }
}

/// Which side of the line `a` to `b` the point `p` lies on: positive to the
/// left, negative to the right, zero on the line.
fn side(a: Point, b: Point, p: Point) -> i128 {
    let (ax, ay) = (i128::from(a.x.0), i128::from(a.y.0));
    (i128::from(b.x.0) - ax) * (i128::from(p.y.0) - ay)
        - (i128::from(b.y.0) - ay) * (i128::from(p.x.0) - ax)
}

/// Whether `p`, known to lie on the line `a` to `b`, lies between them.
fn between(a: Point, b: Point, p: Point) -> bool {
    p.x.0 >= a.x.0.min(b.x.0)
        && p.x.0 <= a.x.0.max(b.x.0)
        && p.y.0 >= a.y.0.min(b.y.0)
        && p.y.0 <= a.y.0.max(b.y.0)
}

/// Whether the segments `a0 a1` and `b0 b1` share a point, ends included.
fn segments_meet(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
    let d1 = side(a0, a1, b0).signum();
    let d2 = side(a0, a1, b1).signum();
    let d3 = side(b0, b1, a0).signum();
    let d4 = side(b0, b1, a1).signum();
    if d1 * d2 < 0 && d3 * d4 < 0 {
        return true;
    }
    (d1 == 0 && between(a0, a1, b0))
        || (d2 == 0 && between(a0, a1, b1))
        || (d3 == 0 && between(b0, b1, a0))
        || (d4 == 0 && between(b0, b1, a1))
}

/// The box that just holds every corner, or `None` for no corners.
pub fn bounds(points: &[Point]) -> Option<Rect> {
    let first = points.first()?;
    let mut min = *first;
    let mut max = *first;
    for p in &points[1..] {
        min = Point::new(Nm(min.x.0.min(p.x.0)), Nm(min.y.0.min(p.y.0)));
        max = Point::new(Nm(max.x.0.max(p.x.0)), Nm(max.y.0.max(p.y.0)));
    }
    Some(Rect::new(min, max))
}

/// Whether the ring is simple: no edge crosses, touches or runs along another
/// except the neighbour it shares a corner with.
///
/// A ring that is not simple has no one area. KiCad draws zones as the user
/// clicks them and some cross themselves; read by the even-odd rule such a ring
/// is a different shape from the one its winding gives, and copper cut from it
/// would be one of the two by accident. A repeated corner in a row is one corner.
pub fn is_simple(points: &[Point]) -> bool {
    let mut ring: Vec<Point> = open(points).to_vec();
    ring.dedup();
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let dot = |o: Point, a: Point, b: Point| {
        (i128::from(a.x.0) - i128::from(o.x.0)) * (i128::from(b.x.0) - i128::from(o.x.0))
            + (i128::from(a.y.0) - i128::from(o.y.0)) * (i128::from(b.y.0) - i128::from(o.y.0))
    };
    for i in 0..n {
        let (a0, a1) = (ring[i], ring[(i + 1) % n]);
        for j in (i + 1)..n {
            let (b0, b1) = (ring[j], ring[(j + 1) % n]);
            let next_to = j == i + 1 || (i == 0 && j == n - 1);
            if next_to {
                // They share one corner. Folding back on each other is the only
                // way two edges that share a corner can share more.
                let (corner, far_a, far_b) = if j == i + 1 {
                    (a1, a0, b1)
                } else {
                    (a0, a1, b0)
                };
                if side(corner, far_a, far_b) == 0 && dot(corner, far_a, far_b) > 0 {
                    return false;
                }
            } else if segments_meet(a0, a1, b0, b1) {
                return false;
            }
        }
    }
    true
}

/// Whether `p` is inside the ring or on its edge.
///
/// An edge and a corner count as inside, the way [`Rect::contains`] counts
/// them, so a ring that is a rectangle answers as that rectangle does. A
/// notch of a concave ring is outside.
pub fn contains(points: &[Point], p: Point) -> bool {
    let ring = open(points);
    if ring.len() < 3 {
        return false;
    }
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if side(a, b, p) == 0 && between(a, b, p) {
            return true;
        }
    }
    // Crossing number: a ray to the right of `p`, an edge counted when it
    // spans the ray's height by the half-open rule, so a corner on the ray is
    // counted once and not twice.
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a.y.0 > p.y.0) != (b.y.0 > p.y.0) {
            // The edge crosses the ray's height: is `p` left of the crossing?
            let up = b.y.0 > a.y.0;
            let left_of_edge = side(a, b, p) > 0;
            if left_of_edge == up {
                inside = !inside;
            }
        }
    }
    inside
}

/// Whether the ring and the rectangle share a point, a touch included.
pub fn meets_rect(points: &[Point], rect: Rect) -> bool {
    let ring = open(points);
    if ring.len() < 3 {
        return false;
    }
    if ring.iter().any(|p| rect.contains(*p)) {
        return true;
    }
    let corners = rect.corners();
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        for j in 0..4 {
            if segments_meet(a, b, corners[j], corners[(j + 1) % 4]) {
                return true;
            }
        }
    }
    // No corner of the ring in the rectangle and no edge across it: the
    // rectangle is wholly inside the ring or wholly apart from it.
    contains(ring, rect.min)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> Point {
        Point::new(Nm(x), Nm(y))
    }

    fn ring(points: &[(i64, i64)]) -> Vec<Point> {
        points.iter().map(|&(x, y)| p(x, y)).collect()
    }

    /// An L: the square 0..10 with its upper right quarter, 5..10 by 5..10, cut away.
    fn l_shape() -> Vec<Point> {
        ring(&[(0, 0), (10, 0), (10, 5), (5, 5), (5, 10), (0, 10)])
    }

    #[test]
    fn a_simple_ring_is_simple() {
        assert!(is_simple(&l_shape()));
        assert!(is_simple(&ring(&[(5, 0), (10, 5), (5, 10), (0, 5)])));
        let mut closed = l_shape();
        closed.push(closed[0]);
        assert!(is_simple(&closed), "a closing corner changes nothing");
    }

    #[test]
    fn a_ring_that_crosses_itself_is_not_simple() {
        assert!(
            !is_simple(&ring(&[(0, 0), (10, 10), (10, 0), (0, 10)])),
            "a bow tie"
        );
        assert!(
            !is_simple(&ring(&[(0, 0), (10, 0), (10, 10), (5, -5), (0, 10)])),
            "a star"
        );
    }

    #[test]
    fn a_ring_that_touches_itself_is_not_simple() {
        // A corner of the ring on one of its own edges.
        assert!(!is_simple(&ring(&[
            (0, 0),
            (10, 0),
            (10, 10),
            (5, 0),
            (0, 10)
        ])));
        // Two corners that are one point, the neck of a figure of eight.
        assert!(!is_simple(&ring(&[
            (0, 0),
            (10, 10),
            (20, 0),
            (20, 20),
            (10, 10),
            (0, 20)
        ])));
    }

    #[test]
    fn a_ring_that_doubles_back_on_an_edge_is_not_simple() {
        assert!(
            !is_simple(&ring(&[(0, 0), (10, 0), (5, 0), (5, 10)])),
            "out and back"
        );
    }

    #[test]
    fn a_corner_in_the_middle_of_an_edge_is_still_simple() {
        assert!(is_simple(&ring(&[
            (0, 0),
            (5, 0),
            (10, 0),
            (10, 10),
            (0, 10)
        ])));
    }

    #[test]
    fn a_ring_that_is_one_edge_out_and_back_is_not_simple() {
        assert!(!is_simple(&ring(&[(0, 0), (10, 0), (5, 0)])));
    }

    #[test]
    fn a_repeated_corner_is_one_corner() {
        assert!(is_simple(&ring(&[
            (0, 0),
            (10, 0),
            (10, 0),
            (10, 10),
            (0, 10)
        ])));
    }

    #[test]
    fn a_point_inside_is_inside() {
        assert!(contains(&l_shape(), p(2, 2)));
        assert!(contains(&l_shape(), p(8, 2)));
        assert!(contains(&l_shape(), p(2, 8)));
    }

    #[test]
    fn a_point_on_an_edge_is_inside() {
        let l = l_shape();
        assert!(contains(&l, p(5, 0)), "on the bottom edge");
        assert!(contains(&l, p(0, 5)), "on the left edge");
        assert!(contains(&l, p(10, 3)), "on the right edge");
        assert!(contains(&l, p(7, 5)), "on the edge of the notch");
        assert!(contains(&l, p(5, 7)), "on the other edge of the notch");
    }

    #[test]
    fn a_corner_is_inside() {
        let l = l_shape();
        assert!(contains(&l, p(0, 0)));
        assert!(contains(&l, p(5, 5)), "the corner that turns inward");
        assert!(contains(&l, p(5, 10)));
    }

    #[test]
    fn a_point_in_the_notch_is_outside() {
        assert!(!contains(&l_shape(), p(8, 8)));
        assert!(!contains(&l_shape(), p(6, 6)));
    }

    #[test]
    fn a_point_beyond_the_bounding_box_is_outside() {
        let l = l_shape();
        assert!(!contains(&l, p(-1, 5)));
        assert!(!contains(&l, p(11, 2)));
        assert!(!contains(&l, p(3, 11)));
    }

    #[test]
    fn a_ray_through_a_corner_is_counted_once() {
        // Level with the corner (5, 5) and left of the whole shape.
        assert!(!contains(&l_shape(), p(-3, 5)));
        // Level with the corner (10, 5), inside and to its left.
        assert!(contains(&l_shape(), p(9, 5)));
    }

    #[test]
    fn a_diamond_holds_its_middle_and_not_its_box_corner() {
        let d = ring(&[(5, 0), (10, 5), (5, 10), (0, 5)]);
        assert!(contains(&d, p(5, 5)));
        assert!(contains(&d, p(7, 5)));
        assert!(
            contains(&d, p(8, 3)),
            "on the slanted edge: 10,5 to 5,0 passes 8,3"
        );
        assert!(!contains(&d, p(9, 1)), "the box corner is outside");
    }

    #[test]
    fn a_closing_corner_that_repeats_the_first_changes_nothing() {
        let mut closed = l_shape();
        closed.push(closed[0]);
        for probe in [p(2, 2), p(8, 8), p(5, 0), p(-1, -1), p(10, 5)] {
            assert_eq!(
                contains(&closed, probe),
                contains(&l_shape(), probe),
                "{probe:?}"
            );
        }
    }

    #[test]
    fn fewer_than_three_corners_hold_nothing() {
        assert!(!contains(&ring(&[(0, 0), (10, 0)]), p(5, 0)));
        assert!(!contains(&[], p(0, 0)));
    }

    #[test]
    fn a_rectangle_answers_as_the_rectangle_does() {
        let r = Rect::new(p(0, 0), p(10, 6));
        let ring = ring(&[(0, 0), (10, 0), (10, 6), (0, 6)]);
        for x in -2..=12 {
            for y in -2..=8 {
                assert_eq!(contains(&ring, p(x, y)), r.contains(p(x, y)), "({x}, {y})");
            }
        }
    }

    #[test]
    fn the_bounds_hold_every_corner() {
        assert_eq!(bounds(&l_shape()), Some(Rect::new(p(0, 0), p(10, 10))));
        assert_eq!(bounds(&[]), None);
    }

    #[test]
    fn a_rectangle_in_the_notch_does_not_meet_the_ring() {
        let notch = Rect::new(p(7, 7), p(9, 9));
        assert!(!meets_rect(&l_shape(), notch));
    }

    #[test]
    fn a_rectangle_that_crosses_an_edge_meets_the_ring() {
        let across = Rect::new(p(4, 4), p(6, 6));
        assert!(meets_rect(&l_shape(), across), "over the inward corner");
        let over_the_edge = Rect::new(p(-3, 1), p(2, 2));
        assert!(meets_rect(&l_shape(), over_the_edge));
    }

    #[test]
    fn a_rectangle_inside_the_ring_meets_it() {
        assert!(meets_rect(&l_shape(), Rect::new(p(1, 1), p(2, 2))));
    }

    #[test]
    fn a_rectangle_round_the_ring_meets_it() {
        assert!(meets_rect(&l_shape(), Rect::new(p(-5, -5), p(20, 20))));
    }

    #[test]
    fn a_rectangle_that_only_touches_meets_the_ring() {
        // Touching along the edge x = 10, and at the corner (10, 5).
        assert!(meets_rect(&l_shape(), Rect::new(p(10, 1), p(12, 3))));
        assert!(meets_rect(&l_shape(), Rect::new(p(10, 5), p(12, 8))));
    }

    #[test]
    fn a_rectangle_apart_does_not_meet_the_ring() {
        assert!(!meets_rect(&l_shape(), Rect::new(p(11, 1), p(12, 3))));
        assert!(!meets_rect(&l_shape(), Rect::new(p(6, 6), p(9, 9))));
    }

    #[test]
    fn a_long_thin_rectangle_across_a_diagonal_edge_meets_the_ring() {
        // Neither of its ends is in the ring and no corner of the ring is in it.
        let d = ring(&[(5, 0), (10, 5), (5, 10), (0, 5)]);
        let thin = Rect::new(p(-5, 4), p(15, 5));
        assert!(meets_rect(&d, thin));
        let apart = Rect::new(p(-5, 0), p(1, 1));
        assert!(
            !meets_rect(&d, apart),
            "the box corner of the diamond is not in it"
        );
    }
}
