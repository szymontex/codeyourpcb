//! Is the copper of a net still one piece after the smoother has moved it?
//!
//! `cargo test -p cypcb-autoroute --test the_smoother_keeps_every_joint`
//!
//! With `stop_at_own_copper` a later path of a net stops on the copper an
//! earlier one laid, and three pieces meet at that point. The smoother used to
//! see only the two that one run carries through, so it cut the corner
//! between them and left the third piece behind. On `mains-sequencer` that
//! cut PE and GND in two, where the same board without smoothing had no net
//! in pieces. Both shapes of such a joint are rebuilt here: three ends in one
//! point, and one end on the inside of a segment.

use cypcb_autoroute::smoother::smooth_routes;
use cypcb_core::{Nm, Point};
use cypcb_router::types::RouteSegment;
use cypcb_world::{Layer, NetId};

/// The router's default `roundness`.
const ROUNDNESS: f64 = 0.5;

fn segment(from: (f64, f64), to: (f64, f64)) -> RouteSegment {
    RouteSegment::new(
        NetId::new(1),
        Layer::TopCopper,
        Nm::from_mm(0.127),
        Point::from_mm(from.0, from.1),
        Point::from_mm(to.0, to.1),
    )
}

/// Distance in nanometres from `p` to the segment `a`-`b`.
fn distance(p: Point, a: Point, b: Point) -> f64 {
    let (p, a, b) = (
        [p.x.0 as f64, p.y.0 as f64],
        [a.x.0 as f64, a.y.0 as f64],
        [b.x.0 as f64, b.y.0 as f64],
    );
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length_sq = dx * dx + dy * dy;
    let t = if length_sq == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length_sq).clamp(0.0, 1.0)
    };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

/// How many pieces the copper is in, joining two segments where an end of
/// one lies on the other.
fn pieces(segments: &[RouteSegment]) -> usize {
    let mut piece: Vec<usize> = (0..segments.len()).collect();
    fn root(piece: &mut [usize], mut at: usize) -> usize {
        while piece[at] != at {
            at = piece[at];
        }
        at
    }
    for (i, one) in segments.iter().enumerate() {
        for (j, other) in segments.iter().enumerate().skip(i + 1) {
            let touches = [one.start, one.end]
                .iter()
                .any(|p| distance(*p, other.start, other.end) < 1.0)
                || [other.start, other.end]
                    .iter()
                    .any(|p| distance(*p, one.start, one.end) < 1.0);
            if touches {
                let (a, b) = (root(&mut piece, i), root(&mut piece, j));
                piece[a] = b;
            }
        }
    }
    (0..segments.len())
        .filter(|&i| root(&mut piece, i) == i)
        .count()
}

#[test]
fn three_ends_in_one_point_stay_together() {
    // PE on `mains-sequencer`: along to the joint and down from it, then a
    // later path that stopped on the joint and goes up. The first two make a
    // 90 degree corner the smoother chamfers when nothing holds the joint.
    let segments = [
        segment((60.9765, 14.0715), (70.3575, 14.0715)),
        segment((70.3575, 14.0715), (70.3575, 23.4525)),
        segment((70.3575, 14.0715), (70.3575, 4.6905)),
    ];
    assert_eq!(pieces(&segments), 1);

    let smoothed = smooth_routes(&segments, &[], &[], Nm(0), ROUNDNESS);

    assert_eq!(
        pieces(&smoothed),
        1,
        "the joint came apart after smoothing:\n{smoothed:#?}"
    );
}

#[test]
fn an_end_on_the_inside_of_a_segment_stays_on_it() {
    // A staircase whose first step runs through the point where a later path
    // stopped. The smoother collapses the staircase into a diagonal from
    // (0, 0) when nothing holds that point.
    let segments = [
        segment((0.0, 0.0), (14.0, 0.0)),
        segment((14.0, 0.0), (14.0, 1.0)),
        segment((14.0, 1.0), (15.0, 1.0)),
        segment((12.4, -3.0), (12.4, 0.0)),
    ];
    assert_eq!(pieces(&segments), 1);

    let smoothed = smooth_routes(&segments, &[], &[], Nm(0), ROUNDNESS);

    assert_eq!(
        pieces(&smoothed),
        1,
        "the path that stopped on the staircase came off it:\n{smoothed:#?}"
    );
}
