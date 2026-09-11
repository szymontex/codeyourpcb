//! Does a violation that measured a distance report the distance it measured?
//!
//! `cargo test -p cypcb-drc --test a_measured_fault_carries_its_measurement -- --nocapture`
//!
//! `cypcb check` ranks violations worst-first by how far under its rule each row
//! is, and a row with no number sorts to the end on purpose: an unrouted pin and
//! an assertion measure nothing, and a number invented for them would sort them
//! among the ones that have one.
//!
//! Two constructors took the measurement as a parameter, printed it into the
//! message and then set the field to `None` - so a hole 0.05 mm from another
//! where 0.15 mm was required, a two-thirds miss, sorted below a trace that
//! missed by five percent. Neither produced a compiler warning, because the
//! parameters were used: in the message, where no ranking can see them.

use bevy_ecs::entity::Entity;
use cypcb_core::{Nm, Point};
use cypcb_drc::{shortfall, DrcViolation, ViolationKind};

fn here() -> Point {
    Point::from_mm(10.0, 20.0)
}

fn two() -> (Entity, Entity) {
    (Entity::from_raw(1), Entity::from_raw(2))
}

/// The ranking `cypcb check` applies, copied rather than imported because the
/// CLI is a binary. A row with no number gets -1.0 and lands at the end.
fn ranked(mut violations: Vec<DrcViolation>) -> Vec<DrcViolation> {
    violations.sort_by(|left, right| {
        let key = |violation: &DrcViolation| shortfall(violation).unwrap_or(-1.0);
        key(right)
            .partial_cmp(&key(left))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    violations
}

#[test]
fn a_hole_too_close_to_another_reports_how_close() {
    let (a, b) = two();
    let violation = DrcViolation::hole_to_hole(a, b, Nm::from_mm(0.05), Nm::from_mm(0.15), here());

    assert_eq!(violation.actual, Some(Nm::from_mm(0.05)));
    assert_eq!(violation.required, Some(Nm::from_mm(0.15)));

    let missed = shortfall(&violation).expect("a measured fault has a shortfall");
    println!("hole_to_hole 0.05mm against 0.15mm: {missed:.3}");
    assert!(
        (missed - 2.0 / 3.0).abs() < 1e-9,
        "0.05mm where 0.15mm was required is a two-thirds miss, got {missed}"
    );
}

#[test]
fn a_mask_bridge_reports_how_thin_it_is() {
    let (a, b) = two();
    let violation =
        DrcViolation::solder_mask_bridge(a, b, Nm::from_mm(0.02), Nm::from_mm(0.10), here());

    assert_eq!(violation.actual, Some(Nm::from_mm(0.02)));
    assert_eq!(violation.required, Some(Nm::from_mm(0.10)));

    let missed = shortfall(&violation).expect("a measured fault has a shortfall");
    println!("solder_mask_bridge 0.02mm against 0.10mm: {missed:.3}");
    assert!(
        (missed - 0.8).abs() < 1e-9,
        "0.02mm where 0.10mm was required is an eighty percent miss, got {missed}"
    );
}

#[test]
fn the_worse_fault_is_read_first() {
    // The consequence, not the field. Three rows: a hole that missed by two
    // thirds, a mask bridge that missed by four fifths, and a trace that missed
    // by five percent. Worst first means the bridge, then the hole, then the
    // trace - and before this change both of the first two sorted last, because
    // a row with no number is put at the end deliberately.
    let (a, b) = two();
    let rows = vec![
        DrcViolation::trace_width(a, Nm::from_mm(0.1207), Nm::from_mm(0.127), here()),
        DrcViolation::hole_to_hole(a, b, Nm::from_mm(0.05), Nm::from_mm(0.15), here()),
        DrcViolation::solder_mask_bridge(a, b, Nm::from_mm(0.02), Nm::from_mm(0.10), here()),
    ];

    let order: Vec<ViolationKind> = ranked(rows).iter().map(|row| row.kind).collect();
    println!("worst first: {order:?}");

    assert_eq!(
        order,
        vec![
            ViolationKind::SolderMaskBridge,
            ViolationKind::HoleToHole,
            ViolationKind::TraceWidth,
        ],
        "the two measured-but-silent kinds have to rank on how badly they missed, \
         not fall to the end beside the faults that measure nothing"
    );
}

#[test]
fn a_fault_with_nothing_to_measure_still_has_no_number() {
    // The control on the other side. This change must not hand a number to a
    // kind that has none, because the ranking relies on those staying at the
    // end - that is what the comment in `cypcb check` says and what the earlier
    // paste-stencil incident cost. An unrouted pin measures nothing.
    let violation = DrcViolation::unrouted_pin(Entity::from_raw(3), "1", "R1", here());

    assert_eq!(
        violation.actual, None,
        "an unrouted pin measures no distance"
    );
    assert!(
        shortfall(&violation).is_none(),
        "and so it has no shortfall to rank on"
    );

    // And it sorts behind a row that does have one, whichever order they arrive.
    let (a, b) = two();
    let rows = vec![
        violation,
        DrcViolation::hole_to_hole(a, b, Nm::from_mm(0.14), Nm::from_mm(0.15), here()),
    ];
    let order: Vec<ViolationKind> = ranked(rows).iter().map(|row| row.kind).collect();
    assert_eq!(
        order,
        vec![ViolationKind::HoleToHole, ViolationKind::UnroutedPin],
        "a hole that missed by seven percent still outranks a fault with no number"
    );
}
