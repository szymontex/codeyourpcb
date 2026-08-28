//! The curve reaches the thing that draws.
//!
//! `cargo test -p cypcb-render --test the_curve_reaches_the_screen`
//!
//! Copper reaches the viewer as segments because segments are what everything
//! here measures, and that is right for the checker and for a hit test. It is
//! wrong for a picture: a canvas draws an arc in one call, and a dozen chords
//! at a high zoom look like a dozen chords on something the design says is
//! smooth. The snapshot carries the curve beside the copper so the screen can
//! draw what the board states.

use cypcb_render::PcbEngine;

/// A quarter turn of 4mm radius, clockwise, between two straight runs.
const BOARD: &str = r#"version 1

board curved {
    size 24mm x 20mm
    layers 2
}

net SIG {
}

trace SIG {
    layer top
    width 0.25mm
    path 8mm, 6mm -> 12mm, 6mm
    arc centre 12mm, 10mm sweep 90 clockwise
}
"#;

/// The same copper with nothing curved about it.
const STRAIGHT: &str = r#"version 1

board plain {
    size 24mm x 20mm
    layers 2
}

net SIG {
}

trace SIG {
    layer top
    width 0.25mm
    path 8mm, 6mm -> 12mm, 6mm
}
"#;

fn snapshot(source: &str) -> cypcb_render::BoardSnapshot {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(source);
    assert!(errors.is_empty(), "{errors}");
    engine.build_snapshot()
}

#[test]
fn the_curve_arrives_with_the_copper_it_became() {
    let snapshot = snapshot(BOARD);
    let curved: Vec<_> = snapshot
        .traces
        .iter()
        .filter(|trace| trace.curve.is_some())
        .collect();
    assert_eq!(curved.len(), 1, "one curve was written, one arrives");

    let curve = curved[0].curve.expect("the curve");
    assert_eq!(curve.centre_x, 12_000_000.0, "the centre it turns about");
    assert_eq!(curve.centre_y, 10_000_000.0);
    assert!(
        (curve.radius - 4_000_000.0).abs() < 1_000.0,
        "at the radius it turns at: {}",
        curve.radius
    );
    assert!(
        (curve.start_degrees + 90.0).abs() < 0.01,
        "starting due south of the centre: {}",
        curve.start_degrees
    );
    assert!(
        (curve.sweep_degrees + 90.0).abs() < 0.01,
        "and turning a quarter the way the board says: {}",
        curve.sweep_degrees
    );
}

#[test]
fn the_copper_is_still_the_copper() {
    // The curve is beside the segments rather than instead of them: a hit test
    // walks the chords, and a viewer that only knew the arc could not tell
    // whether a click landed on copper.
    let snapshot = snapshot(BOARD);
    let curved = snapshot
        .traces
        .iter()
        .find(|trace| trace.curve.is_some())
        .expect("the curve");
    assert!(
        curved.segments.len() >= 8,
        "the chords are still there: {}",
        curved.segments.len()
    );
}

#[test]
fn copper_that_is_not_a_curve_says_nothing_about_one() {
    let snapshot = snapshot(STRAIGHT);
    assert!(
        snapshot.traces.iter().all(|trace| trace.curve.is_none()),
        "a straight run reaches the screen exactly as it did"
    );
}
