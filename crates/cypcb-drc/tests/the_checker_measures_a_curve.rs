//! The checker measures a curve, because a curve arrives as chords.
//!
//! `cargo test -p cypcb-drc --test the_checker_measures_a_curve`
//!
//! Row 2 of the KiCad parity audit - arcs in copper - was deferred on
//! 2026-08-27 for one reason: `segment_distance` measures straight segments,
//! so an arc in the model would be copper the checker could not measure. This
//! is the test that says the reason is gone. Nothing here is in the language;
//! the arc is flattened and spawned as the copper it stands for, and the
//! clearance rule is asked what it sees.

use cypcb_core::{Nm, Point};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{ClearanceRule, DrcRule};
use cypcb_world::arc::Arc;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{Layer, NetId};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// A quarter turn of 5mm radius centred at 10mm, 10mm: its rightmost copper is
/// at x = 15mm, y = 10mm.
fn quarter() -> Arc {
    Arc {
        centre: Point::from_mm(10.0, 10.0),
        radius: Nm::from_mm(5.0),
        start_millideg: 0,
        sweep_millideg: 90_000,
    }
}

/// A board carrying the flattened arc and one straight track beside it.
///
/// The track runs parallel to the arc's tangent at 45 degrees and `standoff`
/// millimetres outside the curve, so the closest approach is in the **middle**
/// of the arc rather than at either end. A track placed off the arc's
/// rightmost point would be measured against a chord end, where a flattening
/// is exact and a bad one would look fine.
fn board_with_curve_and_track(standoff: f64) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(30.0)), 2);

    let curve = quarter().flatten(Arc::DEFAULT_TOLERANCE);
    let segments: Vec<TraceSegment> = curve
        .windows(2)
        .map(|pair| TraceSegment::new(pair[0], pair[1]))
        .collect();
    let arc_net = NetId::new(1);
    world.spawn_entity((
        Trace {
            segments,
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: arc_net,
            locked: false,
            source: TraceSource::Autorouted,
        },
        arc_net,
    ));

    // 45 degrees out from the centre, then 3mm each way along the tangent.
    let out = (5.0 + standoff) * std::f64::consts::FRAC_1_SQRT_2;
    let along = 3.0 * std::f64::consts::FRAC_1_SQRT_2;
    let straight_net = NetId::new(2);
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(10.0 + out - along, 10.0 + out + along),
                Point::from_mm(10.0 + out + along, 10.0 + out - along),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: straight_net,
            locked: false,
            source: TraceSource::Autorouted,
        },
        straight_net,
    ));

    let library = FootprintLibrary::new();
    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

/// The tightest gap the checker reported.
fn closest(violations: &[cypcb_drc::DrcViolation]) -> Nm {
    violations
        .iter()
        .filter_map(|violation| violation.actual)
        .min()
        .expect("a clearance violation measures")
}

#[test]
fn a_curve_far_enough_from_a_track_is_measured_as_clean() {
    // Half a millimetre between the centre lines, less 0.1mm of half-width
    // each side: 0.3mm of gap, well over the 0.127mm JLCPCB minimum.
    let (mut world, _library) = board_with_curve_and_track(0.5);
    let violations = ClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());
    assert!(
        violations.is_empty(),
        "0.3mm of gap is not a violation, and the checker said: {:?}",
        violations
            .iter()
            .map(|v| v.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_curve_too_close_is_reported_at_the_distance_it_really_is() {
    // A quarter of a millimetre between the centre lines, less 0.1mm of
    // half-width each side: 0.05mm of gap, under the minimum. The number the
    // checker reports is the whole point - a flattened curve that measured
    // 0.00 or 0.10 would send a board out on a figure nobody could act on.
    let (mut world, _library) = board_with_curve_and_track(0.25);
    let violations = ClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());

    // Several chords stand where the curve passes closest, so several of them
    // are under the minimum: the reading that matters is the tightest.
    assert!(
        !violations.is_empty(),
        "copper this close has to be reported"
    );
    let actual = closest(&violations);
    let error = (actual.0 - Nm::from_mm(0.05).0).abs();
    assert!(
        error <= Arc::DEFAULT_TOLERANCE.0,
        "the gap is 0.05mm and the checker measured {:.4}mm - {error} nanometres out, \
         with {} allowed by the flattening",
        actual.to_mm(),
        Arc::DEFAULT_TOLERANCE.0
    );
}

#[test]
fn the_chords_are_what_the_checker_reads_and_they_are_inside_the_curve() {
    // The direction of the error matters as much as its size. A chord cuts the
    // corner, so a flattened arc is always a little further from a neighbour
    // than the true curve is - the checker under-reports the risk by at most
    // the tolerance and never over-reports the copper.
    let arc = quarter();
    let (mut world, _library) = board_with_curve_and_track(0.25);
    let violations = ClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());
    let actual = closest(&violations);

    assert!(
        actual.0 >= Nm::from_mm(0.05).0,
        "a chord is inside the curve, so the gap it reports is never smaller \
         than the true one: {:.4}mm against 0.0500mm",
        actual.to_mm()
    );
    assert!(
        actual.0 - Nm::from_mm(0.05).0 <= arc.chord_error(Arc::DEFAULT_TOLERANCE).0,
        "and it is out by no more than the arc said it would be: {:?}",
        arc.chord_error(Arc::DEFAULT_TOLERANCE)
    );
}
