//! Copper cut from a ring: the pieces a polygon pour becomes.
//!
//! [`crate::pour`] makes a rectangular pour out of rectangles. A pour drawn as a
//! polygon is cut the same way - the ring minus every piece of foreign copper,
//! each grown by the clearance - and the copper that is left is not a set of
//! rectangles, because the ring's own edges are not axis-aligned. What is left
//! is convex pieces, here a ring of corners each.
//!
//! The ring is first cut into horizontal slabs, one at every height a corner of
//! the ring stands at, and each slab into trapezoids by the even-odd rule. An
//! obstacle is always a rectangle, so taking it out of a convex piece is four
//! half-plane cuts - below it, above it, and to the left and right of it in
//! its own band - the way [`crate::pour::fill`] takes it out of a rectangle.
//!
//! Every corner is a whole nanometre. A cut that falls on a slanted edge at a
//! fractional nanometre is rounded to the side that is inside the piece being
//! cut, so the copper that results is inside the ring and outside the clearance
//! by construction, never by a tolerance: the error is under one nanometre of
//! copper missing at each such corner, and none of it extra.

use crate::{Nm, Point, Rect};

/// Twice the signed area of the ring, positive for counter-clockwise.
pub fn twice_area(ring: &[Point]) -> i128 {
    (0..ring.len())
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            i128::from(a.x.0) * i128::from(b.y.0) - i128::from(b.x.0) * i128::from(a.y.0)
        })
        .sum()
}

fn cross(o: Point, a: Point, b: Point) -> i128 {
    (i128::from(a.x.0) - i128::from(o.x.0)) * (i128::from(b.y.0) - i128::from(o.y.0))
        - (i128::from(a.y.0) - i128::from(o.y.0)) * (i128::from(b.x.0) - i128::from(o.x.0))
}

fn floor_div(n: i128, d: i128) -> i128 {
    debug_assert!(d > 0);
    n.div_euclid(d)
}

fn ceil_div(n: i128, d: i128) -> i128 {
    debug_assert!(d > 0);
    -((-n).div_euclid(d))
}

fn pt(x: i128, y: i128) -> Point {
    Point::new(Nm(x as i64), Nm(y as i64))
}

/// The convex hull, counter-clockwise, with no repeated and no collinear corner.
/// Fewer than three corners, or no area, gives `None`.
fn hull(points: &[Point]) -> Option<Vec<Point>> {
    let mut p: Vec<Point> = points.to_vec();
    p.sort_by_key(|q| (q.x.0, q.y.0));
    p.dedup();
    if p.len() < 3 {
        return None;
    }
    let mut lower: Vec<Point> = Vec::new();
    for &q in &p {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], q) <= 0 {
            lower.pop();
        }
        lower.push(q);
    }
    let mut upper: Vec<Point> = Vec::new();
    for &q in p.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], q) <= 0 {
            upper.pop();
        }
        upper.push(q);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    (lower.len() >= 3 && twice_area(&lower) > 0).then_some(lower)
}

/// Whether a point is inside a convex counter-clockwise piece or on its edge.
pub fn convex_contains(piece: &[Point], p: Point) -> bool {
    (0..piece.len()).all(|i| cross(piece[i], piece[(i + 1) % piece.len()], p) >= 0)
}

/// The box that just holds the piece.
pub fn shape_bounds(piece: &[Point]) -> Rect {
    crate::outline::bounds(piece).expect("a piece has corners")
}

/// Which axis a cut is made along.
#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

/// The part of a convex piece on one side of a line `axis = at`.
///
/// `keep_low` keeps `axis <= at`, otherwise `axis >= at`. A corner made on a
/// slanted edge is the one of the two whole-nanometre points either side of
/// the exact cut that lies inside `piece`.
fn clip(piece: &[Point], axis: Axis, at: i64, keep_low: bool) -> Option<Vec<Point>> {
    let coord = |p: Point| match axis {
        Axis::X => p.x.0,
        Axis::Y => p.y.0,
    };
    let other = |p: Point| match axis {
        Axis::X => p.y.0,
        Axis::Y => p.x.0,
    };
    let inside = |p: Point| {
        if keep_low {
            coord(p) <= at
        } else {
            coord(p) >= at
        }
    };
    let make = |along: i128| match axis {
        Axis::X => pt(i128::from(at), along),
        Axis::Y => pt(along, i128::from(at)),
    };

    let mut out: Vec<Point> = Vec::with_capacity(piece.len() + 2);
    for i in 0..piece.len() {
        let (p, q) = (piece[i], piece[(i + 1) % piece.len()]);
        if inside(p) {
            out.push(p);
        }
        if inside(p) != inside(q) {
            // The exact cut: `other` runs from p to q as `coord` runs from p to q.
            let (dc, dn) = (
                i128::from(coord(q)) - i128::from(coord(p)),
                i128::from(other(q)) - i128::from(other(p)),
            );
            let mut num = dn * (i128::from(at) - i128::from(coord(p)));
            let mut den = dc;
            if den < 0 {
                num = -num;
                den = -den;
            }
            let base = i128::from(other(p)) * den + num;
            let (low, high) = (floor_div(base, den), ceil_div(base, den));
            let candidates = if low == high {
                vec![low]
            } else {
                vec![low, high]
            };
            let inside_piece: Vec<i128> = candidates
                .into_iter()
                .filter(|&c| convex_contains(piece, make(c)))
                .collect();
            // Both inside: the nearer to the exact cut, the lower on a tie.
            let chosen = match inside_piece.as_slice() {
                [] => None,
                [only] => Some(*only),
                [low, high] => Some(if (base - low * den) * 2 <= den {
                    *low
                } else {
                    *high
                }),
                _ => unreachable!("two candidates at most"),
            };
            if let Some(c) = chosen {
                out.push(make(c));
            }
        }
    }
    hull(&out)
}

/// The part of a convex piece inside a rectangle, or `None` when they share no area.
pub fn clip_to_rect(piece: &[Point], rect: Rect) -> Option<Vec<Point>> {
    let a = clip(piece, Axis::X, rect.min.x.0, false)?;
    let b = clip(&a, Axis::X, rect.max.x.0, true)?;
    let c = clip(&b, Axis::Y, rect.min.y.0, false)?;
    clip(&c, Axis::Y, rect.max.y.0, true)
}

/// A convex piece with a rectangle taken out of it: up to four convex pieces
/// that do not overlap each other or the rectangle.
pub fn cut_box(piece: &[Point], hole: Rect) -> Vec<Vec<Point>> {
    // Asked of the exact corners and not of a clipped copy: a sliver of the
    // piece inside the hole can be thinner than a nanometre, round away, and
    // leave a corner of the piece standing in the clearance.
    if !interiors_overlap(piece, hole) {
        return vec![piece.to_vec()];
    }
    let mut out = Vec::with_capacity(4);
    // Below the hole, and above it.
    out.extend(clip(piece, Axis::Y, hole.min.y.0, true));
    out.extend(clip(piece, Axis::Y, hole.max.y.0, false));
    // The band beside it, split left and right.
    let band = clip(piece, Axis::Y, hole.min.y.0, false)
        .and_then(|upper| clip(&upper, Axis::Y, hole.max.y.0, true));
    if let Some(band) = band {
        out.extend(clip(&band, Axis::X, hole.min.x.0, true));
        out.extend(clip(&band, Axis::X, hole.max.x.0, false));
    }
    out
}

/// The ring as convex pieces, counter-clockwise, that do not overlap.
///
/// The ring may be concave and wound either way. A ring that crosses itself is
/// read by the even-odd rule.
pub fn convex_pieces(ring: &[Point]) -> Vec<Vec<Point>> {
    let ring: Vec<Point> = match ring {
        [first, .., last] if first == last => ring[..ring.len() - 1].to_vec(),
        _ => ring.to_vec(),
    };
    if ring.len() < 3 {
        return Vec::new();
    }
    let mut ys: Vec<i64> = ring.iter().map(|p| p.y.0).collect();
    ys.sort_unstable();
    ys.dedup();

    // Each slanted or vertical edge, lowest end first.
    let edges: Vec<(Point, Point)> = (0..ring.len())
        .filter_map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            match a.y.0.cmp(&b.y.0) {
                std::cmp::Ordering::Less => Some((a, b)),
                std::cmp::Ordering::Greater => Some((b, a)),
                std::cmp::Ordering::Equal => None,
            }
        })
        .collect();

    // x of an edge at a height, as a fraction `(numerator, denominator > 0)`.
    let x_at = |e: &(Point, Point), y2: i128| -> (i128, i128) {
        let (a, b) = *e;
        let dy = i128::from(b.y.0) - i128::from(a.y.0);
        let dx = i128::from(b.x.0) - i128::from(a.x.0);
        // y2 is twice the height, so the fraction has a denominator of 2 dy.
        (
            i128::from(a.x.0) * dy * 2 + dx * (y2 - 2 * i128::from(a.y.0)),
            2 * dy,
        )
    };

    let mut pieces = Vec::new();
    for slab in ys.windows(2) {
        let (lo, hi) = (slab[0], slab[1]);
        let mut across: Vec<&(Point, Point)> = edges
            .iter()
            .filter(|(a, b)| a.y.0 <= lo && b.y.0 >= hi)
            .collect();
        let mid = i128::from(lo) + i128::from(hi);
        across.sort_by(|p, q| {
            let (pn, pd) = x_at(p, mid);
            let (qn, qd) = x_at(q, mid);
            (pn * qd).cmp(&(qn * pd))
        });
        for pair in across.chunks(2) {
            let [left, right] = pair else { continue };
            // The sides' x at a slab's own height: a left side rounds right and
            // a right side rounds left, so the corner is inside the ring.
            let side = |e: &(Point, Point), y: i64, round_up: bool| -> i128 {
                let (n, d) = x_at(e, 2 * i128::from(y));
                if round_up {
                    ceil_div(n, d)
                } else {
                    floor_div(n, d)
                }
            };
            let corners = [
                pt(side(left, lo, true), i128::from(lo)),
                pt(side(right, lo, false), i128::from(lo)),
                pt(side(right, hi, false), i128::from(hi)),
                pt(side(left, hi, true), i128::from(hi)),
            ];
            pieces.extend(hull(&corners));
        }
    }
    pieces
}

/// The ring minus every obstacle grown by `clearance`: convex pieces.
pub fn fill_ring(ring: &[Point], obstacles: &[Rect], clearance: Nm) -> Vec<Vec<Point>> {
    cut_all(convex_pieces(ring), obstacles, clearance)
}

/// Every piece with every obstacle, grown by `clearance`, taken out of it.
pub fn cut_all(mut pieces: Vec<Vec<Point>>, obstacles: &[Rect], clearance: Nm) -> Vec<Vec<Point>> {
    for obstacle in obstacles {
        let hole = Rect {
            min: Point::new(
                Nm(obstacle.min.x.0 - clearance.0),
                Nm(obstacle.min.y.0 - clearance.0),
            ),
            max: Point::new(
                Nm(obstacle.max.x.0 + clearance.0),
                Nm(obstacle.max.y.0 + clearance.0),
            ),
        };
        pieces = cut_pieces(pieces, hole);
        if pieces.is_empty() {
            break;
        }
    }
    pieces
}

/// Every piece with the rectangle taken out of it.
pub fn cut_pieces(pieces: Vec<Vec<Point>>, hole: Rect) -> Vec<Vec<Point>> {
    let mut next = Vec::with_capacity(pieces.len() + 3);
    for piece in pieces {
        let bounds = shape_bounds(&piece);
        // A piece whose box misses the hole is not touched, which is most of them.
        if bounds.max.x.0 <= hole.min.x.0
            || hole.max.x.0 <= bounds.min.x.0
            || bounds.max.y.0 <= hole.min.y.0
            || hole.max.y.0 <= bounds.min.y.0
        {
            next.push(piece);
        } else {
            next.extend(cut_box(&piece, hole));
        }
    }
    next
}

/// The part of a convex piece on one side of the line through `a` and `b`:
/// the left of it (the side a counter-clockwise ring keeps its inside on) or the
/// right. `None` when nothing of the piece with area is left.
///
/// Where the line crosses an edge of the piece the corner is rounded to the
/// nearest nanometre, so a slanted cut can be out by under one.
fn clip_half_plane(piece: &[Point], a: Point, b: Point, keep_left: bool) -> Option<Vec<Point>> {
    let side = |p: Point| -> i128 {
        let s = cross(a, b, p);
        if keep_left {
            s
        } else {
            -s
        }
    };
    let mut kept = Vec::with_capacity(piece.len() + 2);
    for i in 0..piece.len() {
        let (p, q) = (piece[i], piece[(i + 1) % piece.len()]);
        let (sp, sq) = (side(p), side(q));
        if sp >= 0 {
            kept.push(p);
        }
        if (sp > 0 && sq < 0) || (sp < 0 && sq > 0) {
            // p + (q - p) * sp / (sp - sq), in whole nanometres.
            let (num, denom) = if sp > sq {
                (sp, sp - sq)
            } else {
                (-sp, sq - sp)
            };
            let at = |from: i64, to: i64| -> i128 {
                let delta = i128::from(to) - i128::from(from);
                i128::from(from) + (2 * delta * num + denom).div_euclid(2 * denom)
            };
            kept.push(pt(at(p.x.0, q.x.0), at(p.y.0, q.y.0)));
        }
    }
    hull(&kept)
}

/// A convex piece with a convex polygon taken out of it: convex pieces that do
/// not overlap each other or the polygon.
///
/// `hole` is read counter-clockwise whichever way it is wound. Piece by piece
/// the outside of each of its edges is cut away and kept, and what is left
/// inside that edge goes on to the next, so what remains at the end is the
/// part inside every edge - the hole itself, which is dropped.
pub fn cut_convex(piece: &[Point], hole: &[Point]) -> Vec<Vec<Point>> {
    let hole: Vec<Point> = if twice_area(hole) < 0 {
        hole.iter().rev().copied().collect()
    } else {
        hole.to_vec()
    };
    if hole.len() < 3 || twice_area(&hole) == 0 || !convex_interiors_overlap(piece, &hole) {
        return vec![piece.to_vec()];
    }
    let mut out = Vec::with_capacity(hole.len());
    let mut rest = piece.to_vec();
    for i in 0..hole.len() {
        let (a, b) = (hole[i], hole[(i + 1) % hole.len()]);
        out.extend(clip_half_plane(&rest, a, b, false));
        match clip_half_plane(&rest, a, b, true) {
            Some(inside) => rest = inside,
            None => break,
        }
    }
    out
}

/// Every piece with the convex polygon taken out of it.
pub fn cut_pieces_by(pieces: Vec<Vec<Point>>, hole: &[Point]) -> Vec<Vec<Point>> {
    pieces
        .into_iter()
        .flat_map(|piece| cut_convex(&piece, hole))
        .collect()
}

/// Whether two convex pieces share a point. Touching counts, at an edge or at
/// one corner, as it does for two rectangles in the checker.
pub fn shapes_touch(a: &[Point], b: &[Point]) -> bool {
    // Separating axis: some edge of either piece has the other wholly beyond it.
    let apart = |p: &[Point], q: &[Point]| {
        (0..p.len()).any(|i| {
            let (s, e) = (p[i], p[(i + 1) % p.len()]);
            q.iter().all(|&r| cross(s, e, r) < 0)
        })
    };
    !(apart(a, b) || apart(b, a))
}

/// Whether two convex pieces share area, not only a point or an edge.
pub fn convex_interiors_overlap(a: &[Point], b: &[Point]) -> bool {
    // Separating axis, edges counted as separating: the other is wholly on or
    // beyond the line.
    let apart = |p: &[Point], q: &[Point]| {
        (0..p.len()).any(|i| {
            let (s, e) = (p[i], p[(i + 1) % p.len()]);
            q.iter().all(|&r| cross(s, e, r) <= 0)
        })
    };
    !(apart(a, b) || apart(b, a))
}

/// Whether a convex piece and a rectangle share area, not only a point or an edge.
pub fn interiors_overlap(piece: &[Point], rect: Rect) -> bool {
    let ring = [
        rect.min,
        Point::new(rect.max.x, rect.min.y),
        rect.max,
        Point::new(rect.min.x, rect.max.y),
    ];
    if twice_area(&ring) <= 0 {
        return false;
    }
    convex_interiors_overlap(piece, &ring)
}

/// Whether a convex piece and a rectangle share a point.
pub fn shape_touches_rect(piece: &[Point], rect: Rect) -> bool {
    let ring = [
        rect.min,
        Point::new(rect.max.x, rect.min.y),
        rect.max,
        Point::new(rect.min.x, rect.max.y),
    ];
    if twice_area(&ring) <= 0 {
        return false;
    }
    shapes_touch(piece, &ring)
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

    fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
        Rect::new(p(x0, y0), p(x1, y1))
    }

    fn total(pieces: &[Vec<Point>]) -> i128 {
        pieces.iter().map(|q| twice_area(q)).sum()
    }

    fn l() -> Vec<Point> {
        ring(&[
            (0, 0),
            (1000, 0),
            (1000, 500),
            (500, 500),
            (500, 1000),
            (0, 1000),
        ])
    }

    #[test]
    fn the_hull_drops_a_corner_in_the_middle_of_an_edge() {
        let h = hull(&ring(&[
            (0, 0),
            (5, 0),
            (10, 0),
            (10, 10),
            (5, 10),
            (0, 10),
            (0, 5),
        ]))
        .expect("a rectangle");
        assert_eq!(h.len(), 4, "{h:?}");
        assert!(twice_area(&h) > 0, "counter-clockwise");
    }

    #[test]
    fn a_rectangle_is_one_piece_of_its_own_area() {
        let r = ring(&[(0, 0), (1000, 0), (1000, 600), (0, 600)]);
        let pieces = convex_pieces(&r);
        assert_eq!(pieces.len(), 1);
        assert_eq!(total(&pieces), twice_area(&r));
    }

    #[test]
    fn an_l_is_cut_into_pieces_that_add_up_to_it() {
        let pieces = convex_pieces(&l());
        assert_eq!(total(&pieces), twice_area(&l()));
        assert!(pieces.len() >= 2);
    }

    #[test]
    fn the_wind_of_the_ring_does_not_matter() {
        let mut reversed = l();
        reversed.reverse();
        assert_eq!(
            total(&convex_pieces(&l())),
            total(&convex_pieces(&reversed))
        );
    }

    #[test]
    fn every_piece_is_convex_and_counter_clockwise() {
        let star = ring(&[
            (0, 0),
            (700, 300),
            (1000, 0),
            (800, 700),
            (1000, 1000),
            (300, 800),
        ]);
        for piece in convex_pieces(&star) {
            assert!(twice_area(&piece) > 0);
            for i in 0..piece.len() {
                let (a, b, c) = (
                    piece[i],
                    piece[(i + 1) % piece.len()],
                    piece[(i + 2) % piece.len()],
                );
                assert!(cross(a, b, c) > 0, "a turn the wrong way in {piece:?}");
            }
        }
    }

    #[test]
    fn taking_a_box_out_of_a_rectilinear_ring_loses_exactly_the_box() {
        let pieces = fill_ring(&l(), &[rect(100, 100, 300, 400)], Nm(0));
        assert_eq!(total(&pieces), twice_area(&l()) - 2 * 200 * 300);
    }

    #[test]
    fn a_box_that_misses_the_ring_takes_nothing() {
        let before = total(&convex_pieces(&l()));
        assert_eq!(
            total(&fill_ring(&l(), &[rect(600, 600, 900, 900)], Nm(0))),
            before,
            "in the notch"
        );
        assert_eq!(
            total(&fill_ring(&l(), &[rect(2000, 2000, 3000, 3000)], Nm(0))),
            before,
            "far off"
        );
    }

    #[test]
    fn the_clearance_grows_the_box() {
        let pieces = fill_ring(&l(), &[rect(100, 100, 300, 400)], Nm(50));
        assert_eq!(total(&pieces), twice_area(&l()) - 2 * 300 * 400);
    }

    #[test]
    fn a_box_over_everything_leaves_nothing() {
        let t = ring(&[(0, 0), (1000, 0), (0, 1000)]);
        assert!(fill_ring(&t, &[rect(-10, -10, 2000, 2000)], Nm(0)).is_empty());
    }

    /// A triangle whose hypotenuse falls on a fractional nanometre at x = 300.
    fn triangle() -> Vec<Point> {
        ring(&[(0, 0), (1000, 0), (0, 1001)])
    }

    #[test]
    fn a_cut_on_a_slanted_edge_rounds_into_the_ring() {
        // The hypotenuse is y = 1001 * (1000 - x) / 1000, which is 700.7 at x = 300.
        let pieces = fill_ring(&triangle(), &[rect(300, -10, 400, 2000)], Nm(0));
        let left: Vec<&Vec<Point>> = pieces
            .iter()
            .filter(|q| shape_bounds(q).max.x.0 == 300)
            .collect();
        assert_eq!(left.len(), 1, "{pieces:?}");
        let top = left[0]
            .iter()
            .filter(|c| c.x.0 == 300)
            .map(|c| c.y.0)
            .max()
            .unwrap();
        assert_eq!(top, 700, "700.7 rounds down to be inside, not up to 701");
    }

    #[test]
    fn every_corner_is_in_the_ring_and_out_of_the_clearance() {
        // Slanted rings, boxes swept over them, clearances that are not round.
        let rings = [
            triangle(),
            ring(&[(0, 0), (1000, 0), (1000, 1000), (0, 1000)]),
            ring(&[(500, 0), (1000, 500), (500, 1003), (0, 497)]),
            ring(&[
                (0, 0),
                (997, 13),
                (1000, 500),
                (503, 509),
                (491, 1001),
                (3, 997),
            ]),
            ring(&[
                (0, 0),
                (1001, 0),
                (1001, 333),
                (667, 333),
                (667, 1007),
                (0, 1007),
            ]),
        ];
        let mut checked = 0usize;
        for ring in &rings {
            for hx in (-100..1100).step_by(137) {
                for hy in (-100..1100).step_by(173) {
                    let hole = rect(hx, hy, hx + 91, hy + 211);
                    let pieces = fill_ring(ring, &[hole], Nm(7));
                    let grown = rect(hx - 7, hy - 7, hx + 91 + 7, hy + 211 + 7);
                    for piece in &pieces {
                        for &c in piece {
                            assert!(
                                crate::outline::contains(ring, c),
                                "corner {c:?} outside the ring {ring:?}"
                            );
                            let strictly_in = c.x.0 > grown.min.x.0
                                && c.x.0 < grown.max.x.0
                                && c.y.0 > grown.min.y.0
                                && c.y.0 < grown.max.y.0;
                            assert!(!strictly_in, "corner {c:?} inside the clearance {grown:?}");
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(
            checked > 2000,
            "the sweep checked {checked} corners; it has stopped seeing them"
        );
    }

    #[test]
    fn a_sliver_of_the_piece_in_the_clearance_is_still_cut_away() {
        // The tip of a long thin triangle reaches 1 nm into the grown box; the
        // overlap is thinner than a whole nanometre once rounded, and the corner
        // in it must still go.
        let thin = ring(&[(0, 0), (997, 13), (1, 13)]);
        let pieces = fill_ring(&thin, &[rect(1003, -100, 1100, 100)], Nm(7));
        for piece in &pieces {
            assert!(!piece.contains(&p(997, 13)), "{piece:?}");
        }
        assert!(!pieces.is_empty());
    }

    #[test]
    fn the_copper_left_is_the_ring_minus_the_box_to_within_the_rounding() {
        let ring = ring(&[
            (0, 0),
            (997, 13),
            (1000, 500),
            (503, 509),
            (491, 1001),
            (3, 997),
        ]);
        let whole = twice_area(&ring) as f64;
        let pieces = fill_ring(&ring, &[rect(100, 100, 400, 800)], Nm(0));
        let left = total(&pieces) as f64;
        // The box is inside the ring, so what is left is the ring less the box.
        let want = whole - 2.0 * 300.0 * 700.0;
        // A corner moves by less than a nanometre, so a piece loses less than
        // its perimeter in nm2.
        assert!((left - want).abs() < 20_000.0, "left {left}, want {want}");
        assert!(left <= want + 1.0, "rounding only ever removes copper");
    }

    #[test]
    fn the_pieces_of_one_cut_do_not_overlap() {
        let ring = ring(&[
            (0, 0),
            (997, 13),
            (1000, 500),
            (503, 509),
            (491, 1001),
            (3, 997),
        ]);
        let pieces = fill_ring(
            &ring,
            &[rect(200, 150, 650, 600), rect(700, 700, 800, 800)],
            Nm(5),
        );
        let centre = |q: &Vec<Point>| {
            let n = q.len() as i128;
            pt(
                q.iter().map(|c| i128::from(c.x.0)).sum::<i128>() / n,
                q.iter().map(|c| i128::from(c.y.0)).sum::<i128>() / n,
            )
        };
        let strictly_inside = |q: &Vec<Point>, c: Point| {
            (0..q.len()).all(|k| cross(q[k], q[(k + 1) % q.len()], c) > 0)
        };
        for i in 0..pieces.len() {
            for j in (i + 1)..pieces.len() {
                assert!(
                    !strictly_inside(&pieces[i], centre(&pieces[j])),
                    "{:?} holds the middle of {:?}",
                    pieces[i],
                    pieces[j]
                );
                assert!(
                    !strictly_inside(&pieces[j], centre(&pieces[i])),
                    "{:?} holds the middle of {:?}",
                    pieces[j],
                    pieces[i]
                );
            }
        }
    }

    #[test]
    fn clipping_to_a_rectangle_keeps_what_is_inside_it() {
        let t = triangle();
        let clipped = clip_to_rect(&t, rect(0, 0, 500, 500)).expect("they share area");
        for &c in &clipped {
            assert!(c.x.0 >= 0 && c.x.0 <= 500 && c.y.0 >= 0 && c.y.0 <= 500);
            assert!(crate::outline::contains(&t, c));
        }
        assert!(
            clip_to_rect(&t, rect(900, 900, 1000, 1000)).is_none(),
            "the far corner is outside"
        );
    }

    #[test]
    fn two_pieces_that_touch_at_one_corner_touch() {
        let a = ring(&[(0, 0), (100, 0), (0, 100)]);
        let b = ring(&[(100, 0), (200, 0), (200, 100)]);
        assert!(shapes_touch(&a, &b), "sharing the corner (100, 0)");
        let far = ring(&[(101, 0), (200, 0), (200, 100)]);
        assert!(!shapes_touch(&a, &far), "a nanometre apart");
    }

    #[test]
    fn two_convex_pieces_overlap_in_area_and_not_at_an_edge_or_a_corner() {
        let a = ring(&[(0, 0), (100, 0), (100, 100), (0, 100)]);
        assert!(convex_interiors_overlap(
            &a,
            &ring(&[(50, 50), (150, 50), (150, 150), (50, 150)])
        ));
        assert!(
            !convex_interiors_overlap(&a, &ring(&[(100, 0), (200, 0), (200, 100), (100, 100)])),
            "an edge"
        );
        assert!(
            !convex_interiors_overlap(&a, &ring(&[(100, 100), (200, 100), (200, 200), (100, 200)])),
            "a corner"
        );
        assert!(
            !convex_interiors_overlap(&a, &ring(&[(101, 0), (200, 0), (200, 100), (101, 100)])),
            "apart"
        );
        let tri = ring(&[(0, 0), (100, 0), (0, 100)]);
        assert!(
            !convex_interiors_overlap(&tri, &ring(&[(60, 60), (160, 60), (160, 160), (60, 160)])),
            "beyond the slanted edge"
        );
        assert!(convex_interiors_overlap(
            &tri,
            &ring(&[(40, 40), (160, 40), (160, 160), (40, 160)])
        ));
    }

    #[test]
    fn two_pieces_that_share_an_edge_touch_and_two_that_overlap_touch() {
        let a = ring(&[(0, 0), (100, 0), (100, 100), (0, 100)]);
        assert!(shapes_touch(
            &a,
            &ring(&[(100, 0), (200, 0), (200, 100), (100, 100)])
        ));
        assert!(shapes_touch(
            &a,
            &ring(&[(50, 50), (150, 50), (150, 150), (50, 150)])
        ));
        assert!(!shapes_touch(
            &a,
            &ring(&[(101, 0), (200, 0), (200, 100), (101, 100)])
        ));
    }

    /// The answer the checker gives for rectangles, which a polygon has to match.
    fn rect_touches(a: &Rect, b: &Rect) -> bool {
        a.min.x.0 <= b.max.x.0
            && b.min.x.0 <= a.max.x.0
            && a.min.y.0 <= b.max.y.0
            && b.min.y.0 <= a.max.y.0
    }

    #[test]
    fn touching_means_what_it_means_for_two_rectangles() {
        // Every pair from a grid of rectangles, edges and corners on the lines.
        let mut rects = Vec::new();
        for x in [0, 10, 20] {
            for y in [0, 10, 20] {
                for (w, h) in [(10, 10), (5, 15)] {
                    rects.push(rect(x, y, x + w, y + h));
                }
            }
        }
        let as_ring = |r: &Rect| {
            ring(&[
                (r.min.x.0, r.min.y.0),
                (r.max.x.0, r.min.y.0),
                (r.max.x.0, r.max.y.0),
                (r.min.x.0, r.max.y.0),
            ])
        };
        let mut touching = 0;
        for a in &rects {
            for b in &rects {
                assert_eq!(
                    shapes_touch(&as_ring(a), &as_ring(b)),
                    rect_touches(a, b),
                    "{a:?} {b:?}"
                );
                assert_eq!(
                    shape_touches_rect(&as_ring(a), *b),
                    rect_touches(a, b),
                    "{a:?} {b:?}"
                );
                touching += usize::from(rect_touches(a, b));
            }
        }
        assert!(
            touching > 100 && touching < rects.len() * rects.len(),
            "the grid has both kinds of pair"
        );
    }

    fn area2(pieces: &[Vec<Point>]) -> i128 {
        pieces.iter().map(|piece| twice_area(piece)).sum()
    }

    #[test]
    fn a_diamond_taken_out_of_a_square_leaves_the_rest_of_it() {
        let square = ring(&[(0, 0), (10, 0), (10, 10), (0, 10)]);
        let diamond = ring(&[(5, 2), (8, 5), (5, 8), (2, 5)]);
        let left = cut_convex(&square, &diamond);
        // 100 - 18, twice.
        assert_eq!(area2(&left), 2 * 82);
        // None of it inside the diamond.
        for piece in &left {
            assert!(!convex_interiors_overlap(piece, &diamond), "{piece:?}");
        }
    }

    #[test]
    fn a_hole_wound_clockwise_cuts_the_same_copper() {
        let square = ring(&[(0, 0), (10, 0), (10, 10), (0, 10)]);
        let diamond = ring(&[(5, 2), (8, 5), (5, 8), (2, 5)]);
        let mut clockwise = diamond.clone();
        clockwise.reverse();
        assert_eq!(
            area2(&cut_convex(&square, &clockwise)),
            area2(&cut_convex(&square, &diamond))
        );
    }

    #[test]
    fn a_hole_that_misses_the_piece_leaves_it_whole() {
        let square = ring(&[(0, 0), (10, 0), (10, 10), (0, 10)]);
        let far = ring(&[(20, 0), (30, 0), (25, 5)]);
        assert_eq!(cut_convex(&square, &far), vec![square.clone()]);
        // Touching along an edge takes nothing either.
        let beside = ring(&[(10, 0), (20, 0), (20, 10), (10, 10)]);
        assert_eq!(cut_convex(&square, &beside), vec![square]);
    }

    #[test]
    fn a_hole_that_covers_the_piece_leaves_nothing() {
        let small = ring(&[(2, 2), (4, 2), (4, 4), (2, 4)]);
        let big = ring(&[(0, 0), (10, 0), (10, 10), (0, 10)]);
        assert!(cut_convex(&small, &big).is_empty());
    }

    #[test]
    fn a_triangle_cut_from_a_corner_is_the_square_less_the_corner() {
        let square = ring(&[(0, 0), (10, 0), (10, 10), (0, 10)]);
        // Reaches out past the square on two sides: the part inside is the
        // triangle (0,0) (6,0) (0,6), area 18.
        let corner = ring(&[(-5, -5), (6, 0), (0, 6)]);
        let left = cut_convex(&square, &corner);
        let overlap = ring(&[(0, 0), (6, 0), (0, 6)]);
        assert_eq!(area2(&left), 2 * 100 - twice_area(&overlap).abs());
    }
}
