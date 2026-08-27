//! A curve, as chords the rest of this project can already measure.
//!
//! `cargo test -p cypcb-world --test a_curve_becomes_chords`
//!
//! Row 2 of the KiCad parity audit - arcs in copper - was deferred with a
//! measured reason: the checker, the router and both interop paths measure
//! straight segments, so an arc in the model would be copper nothing could
//! check. This is the half that removes the reason, and it is arithmetic
//! before it is a feature: nothing here is in the language yet.

use cypcb_core::{Nm, Point};
use cypcb_world::arc::Arc;

/// A quarter turn of 10mm radius, centred on the origin, starting due east.
fn quarter() -> Arc {
    Arc {
        centre: Point { x: Nm(0), y: Nm(0) },
        radius: Nm(10_000_000),
        start_millideg: 0,
        sweep_millideg: 90_000,
    }
}

/// How far a point is from a centre.
fn radius_of(centre: Point, point: Point) -> f64 {
    let dx = (point.x.0 - centre.x.0) as f64;
    let dy = (point.y.0 - centre.y.0) as f64;
    (dx * dx + dy * dy).sqrt()
}

#[test]
fn the_chords_start_and_stop_where_the_arc_does() {
    // A flattening that misses the ends is copper that does not meet the pad
    // it was drawn to, which is an open circuit rather than a rough curve.
    let arc = quarter();
    let points = arc.flatten(Arc::DEFAULT_TOLERANCE);

    assert_eq!(
        points.first().copied(),
        Some(arc.start()),
        "it starts on the arc"
    );
    assert_eq!(points.last().copied(), Some(arc.end()), "and stops on it");
    assert_eq!(
        arc.start(),
        Point {
            x: Nm(10_000_000),
            y: Nm(0)
        },
        "due east of the centre, a radius out"
    );
    assert_eq!(
        arc.end(),
        Point {
            x: Nm(0),
            y: Nm(10_000_000)
        },
        "and a quarter turn later, due north"
    );
}

#[test]
fn no_chord_cuts_the_corner_by_more_than_it_was_allowed_to() {
    // The whole promise. Every chord's middle is the point furthest inside the
    // true curve, so the sagitta there is the error the flattening admits to.
    let arc = quarter();
    let tolerance = Arc::DEFAULT_TOLERANCE;
    let points = arc.flatten(tolerance);

    let mut worst = 0.0_f64;
    for pair in points.windows(2) {
        let middle = Point {
            x: Nm((pair[0].x.0 + pair[1].x.0) / 2),
            y: Nm((pair[0].y.0 + pair[1].y.0) / 2),
        };
        worst = worst.max(arc.radius.0 as f64 - radius_of(arc.centre, middle));
    }

    assert!(
        worst <= tolerance.0 as f64 + 1.0,
        "the worst chord cut the corner by {worst} nanometres, and {} was allowed",
        tolerance.0
    );
    // And every point is on the circle itself, not merely near it.
    for point in &points {
        let off = (radius_of(arc.centre, *point) - arc.radius.0 as f64).abs();
        assert!(
            off <= 1.0,
            "a chord end sits {off} nanometres off the circle"
        );
    }
}

#[test]
fn the_arc_says_how_far_it_missed_by_before_anybody_measures() {
    // A flattening is only as honest as the error it admits to, and a caller
    // should be able to ask rather than trust.
    let arc = quarter();
    let stated = arc.chord_error(Arc::DEFAULT_TOLERANCE);
    assert!(
        stated <= Arc::DEFAULT_TOLERANCE,
        "the stated error {stated:?} is inside the tolerance asked for"
    );

    let points = arc.flatten(Arc::DEFAULT_TOLERANCE);
    let middle = Point {
        x: Nm((points[0].x.0 + points[1].x.0) / 2),
        y: Nm((points[0].y.0 + points[1].y.0) / 2),
    };
    let measured = arc.radius.0 as f64 - radius_of(arc.centre, middle);
    assert!(
        (measured - stated.0 as f64).abs() <= 2.0,
        "what it said - {stated:?} - is what a chord actually does: {measured}"
    );
}

#[test]
fn a_finer_tolerance_costs_more_chords() {
    let arc = quarter();
    let coarse = arc.chords(Nm(100_000));
    let fine = arc.chords(Nm(1_000));
    assert!(
        fine > coarse,
        "one micron needs more chords than a hundred: {fine} against {coarse}"
    );
    // A tolerance nobody stated is the default rather than none at all.
    assert_eq!(
        arc.chords(Nm(0)),
        arc.chords(Arc::DEFAULT_TOLERANCE),
        "asking for nothing asks for the default"
    );
}

#[test]
fn the_length_is_the_arc_rather_than_the_chord() {
    // A quarter of 10mm radius is 15.70796mm of copper. A length match that
    // measured the chord instead would be 1.4mm short on one corner.
    let arc = quarter();
    let expected = 10_000_000.0 * std::f64::consts::FRAC_PI_2;
    assert!(
        (arc.length().0 as f64 - expected).abs() <= 1_000.0,
        "a quarter turn is {expected} nanometres and it says {:?}",
        arc.length()
    );
}

#[test]
fn the_sign_of_the_sweep_is_which_way_it_turns() {
    // A tool that drops the sign draws the long way round the board.
    let mut clockwise = quarter();
    clockwise.sweep_millideg = -90_000;

    assert_eq!(
        clockwise.end(),
        Point {
            x: Nm(0),
            y: Nm(-10_000_000)
        },
        "the same start turned the other way ends due south"
    );
    let points = clockwise.flatten(Arc::DEFAULT_TOLERANCE);
    assert!(
        points.iter().all(|point| point.y.0 <= 0),
        "and no part of it strays into the half it does not cross"
    );
}

#[test]
fn a_curve_that_is_not_one_is_answered_rather_than_looped_over() {
    let mut nothing = quarter();
    nothing.radius = Nm(0);
    assert_eq!(
        nothing.flatten(Arc::DEFAULT_TOLERANCE),
        vec![nothing.centre],
        "an arc with no radius is its own centre"
    );

    let mut still = quarter();
    still.sweep_millideg = 0;
    assert_eq!(
        still.flatten(Arc::DEFAULT_TOLERANCE),
        vec![still.start()],
        "an arc that turns nowhere is one point"
    );

    let mut whole = quarter();
    whole.sweep_millideg = 360_000;
    let round = whole.flatten(Arc::DEFAULT_TOLERANCE);
    assert!(
        round.len() > 8,
        "a full turn is many chords: {}",
        round.len()
    );
    assert_eq!(
        round.first(),
        round.last(),
        "and it closes back on where it started"
    );
}
