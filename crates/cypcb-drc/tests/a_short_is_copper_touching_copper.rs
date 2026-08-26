//! A short is copper touching copper, and not every zero is a short.
//!
//! `cargo test -p cypcb-drc --test a_short_is_copper_touching_copper`
//!
//! Four places counted "copper touching copper" as *any* violation whose
//! measured distance was zero: `cypcb route`, `cypcb export`, the autorouter's
//! score, and `check` until a 0.000mm paste stencil web turned up in its
//! total. A stencil with no web left is a torn stencil, not a short.
//!
//! That filter was safe only while most rules threw their measurements away.
//! Nine of them - drill size, trace width, edge clearance, annular ring, via
//! diameter, via drill, trace current, silk and courtyard clearance - took an
//! `actual` and a `required`, printed both in the message and stored `None`,
//! so a via with no annular ring could not be counted as a short because
//! nothing knew it measured zero. Now that they carry the numbers, the kind is
//! what keeps the count honest.

use bevy_ecs::entity::Entity;
use cypcb_core::{Nm, Point};
use cypcb_drc::{shorts, DrcViolation};

fn somewhere() -> Point {
    Point::from_mm(10.0, 20.0)
}

#[test]
fn copper_at_no_distance_is_a_short() {
    let touching = DrcViolation::clearance(
        Entity::from_raw(1),
        Entity::from_raw(2),
        Nm::ZERO,
        Nm::from_mm(0.127),
        somewhere(),
    );
    assert_eq!(shorts(std::slice::from_ref(&touching)), 1);

    // 0.05mm is a gap under spec: a yield risk a fab may still build, which is
    // the distinction the sentence beside this number claims to draw.
    let near = DrcViolation::clearance(
        Entity::from_raw(1),
        Entity::from_raw(2),
        Nm::from_mm(0.05),
        Nm::from_mm(0.127),
        somewhere(),
    );
    assert_eq!(shorts(std::slice::from_ref(&near)), 0);
}

#[test]
fn a_rule_that_measured_zero_is_not_copper_touching_copper() {
    let no_ring = DrcViolation::annular_ring(
        Entity::from_raw(3),
        Nm::ZERO,
        Nm::from_mm(0.13),
        somewhere(),
    );
    let no_width = DrcViolation::trace_width(
        Entity::from_raw(4),
        Nm::ZERO,
        Nm::from_mm(0.127),
        somewhere(),
    );

    // The numbers are there - that is the point of the guard.
    assert_eq!(
        no_ring.actual,
        Some(Nm::ZERO),
        "the annular ring rule carries what it measured"
    );
    assert_eq!(
        no_ring.required,
        Some(Nm::from_mm(0.13)),
        "and what it required"
    );
    assert_eq!(
        no_width.actual,
        Some(Nm::ZERO),
        "so does the trace width rule"
    );

    assert_eq!(
        shorts(&[no_ring, no_width]),
        0,
        "a via with no ring and a trace with no width are two faults and no shorts"
    );
}

#[test]
fn a_mixed_report_counts_only_the_touching_pairs() {
    let report = vec![
        DrcViolation::clearance(
            Entity::from_raw(1),
            Entity::from_raw(2),
            Nm::ZERO,
            Nm::from_mm(0.127),
            somewhere(),
        ),
        DrcViolation::clearance(
            Entity::from_raw(3),
            Entity::from_raw(4),
            Nm::ZERO,
            Nm::from_mm(0.127),
            somewhere(),
        ),
        DrcViolation::via_drill(Entity::from_raw(5), Nm::ZERO, Nm::from_mm(0.3), somewhere()),
        DrcViolation::edge_clearance(Entity::from_raw(6), Nm::ZERO, Nm::from_mm(0.2), somewhere()),
    ];
    assert_eq!(shorts(&report), 2, "two of the four are copper on copper");
}
