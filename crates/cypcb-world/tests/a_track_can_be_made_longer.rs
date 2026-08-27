//! The serpentine that makes one track as long as another.
//!
//! `cargo test -p cypcb-world --test a_track_can_be_made_longer`
//!
//! A differential pair only works if both halves arrive together. The checker
//! has measured that skew since it was written and could do nothing about it;
//! this is the shape that can. What matters is that the path really is longer
//! by what was asked, that it still starts and ends where the pads are, and
//! that it stays inside the space it was given.

use cypcb_core::{Nm, Point};
use cypcb_world::meander::{meander, path_length, MeanderSpec};

fn mm(value: f64) -> Nm {
    Nm((value * 1_000_000.0) as i64)
}

fn at(x: f64, y: f64) -> Point {
    Point { x: mm(x), y: mm(y) }
}

fn spec() -> MeanderSpec {
    MeanderSpec {
        amplitude: mm(0.5),
        pitch: mm(1.0),
    }
}

#[test]
fn the_path_is_longer_by_what_was_asked() {
    let start = at(0.0, 0.0);
    let end = at(20.0, 0.0);
    // Deliberately not a whole number of teeth: one tooth adds 1mm here, and
    // 2.5mm of asking is what tells rounding up from rounding down.
    let tuned = meander(start, end, mm(2.5), spec()).expect("20mm of run holds this");

    // One tooth adds twice the amplitude, so the result overshoots by less
    // than one tooth rather than falling short - a pair matched to within a
    // tooth is what a fab's tolerance is stated in anyway.
    assert!(
        tuned.added.0 >= mm(2.5).0,
        "at least what was asked: {} against {}",
        tuned.added.0,
        mm(2.5).0
    );
    assert!(
        tuned.added.0 < mm(2.5).0 + 2 * spec().amplitude.0,
        "and not a tooth more than needed: {}",
        tuned.added.0
    );

    let straight = mm(20.0).0;
    assert_eq!(
        path_length(&tuned.points).0 - straight,
        tuned.added.0,
        "the length it reports is the length it has"
    );
}

#[test]
fn it_still_starts_and_ends_where_the_pads_are() {
    let start = at(3.0, 4.0);
    let end = at(23.0, 4.0);
    let tuned = meander(start, end, mm(2.0), spec()).expect("this run holds a meander");

    assert_eq!(
        tuned.points.first().copied(),
        Some(start),
        "the first point"
    );
    assert_eq!(tuned.points.last().copied(), Some(end), "the last point");
}

#[test]
fn it_stays_within_its_amplitude() {
    let start = at(0.0, 0.0);
    let end = at(20.0, 0.0);
    let tuned = meander(start, end, mm(4.0), spec()).expect("this run holds a meander");

    for point in &tuned.points {
        assert!(
            point.y.0.abs() <= spec().amplitude.0,
            "a corner at {point:?} leaves the space the caller gave"
        );
    }
}

#[test]
fn it_turns_diagonally_too() {
    // The axis is wherever the two points are, not the X axis.
    let start = at(0.0, 0.0);
    let end = at(10.0, 10.0);
    let straight = path_length(&[start, end]).0;
    let tuned = meander(start, end, mm(2.0), spec()).expect("a diagonal run holds one too");

    assert_eq!(tuned.points.first().copied(), Some(start));
    assert_eq!(tuned.points.last().copied(), Some(end));
    assert!(
        path_length(&tuned.points).0 > straight + mm(2.0).0 - 1_000,
        "the diagonal gains its length too"
    );
}

#[test]
fn a_run_too_short_for_a_tooth_gets_nothing() {
    // Better to say no than to fold copper over itself between two pads a
    // millimetre apart.
    let tuned = meander(at(0.0, 0.0), at(1.0, 0.0), mm(10.0), spec());
    assert!(
        tuned.is_none(),
        "10mm cannot be added to a 1mm run at this pitch"
    );
}

#[test]
fn asking_for_nothing_gets_nothing() {
    assert!(
        meander(at(0.0, 0.0), at(10.0, 0.0), Nm(0), spec()).is_none(),
        "a caller that needs no extra length wants the straight line"
    );
}
