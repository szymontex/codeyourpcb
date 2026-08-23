//! What a published violation count counts.
//!
//! `cargo test -p cypcb-drc --test what_a_published_count_counts`
//!
//! The clearance rule reports per pair of segments: two features that touch
//! along a run report once for each segment that takes part, so one contact
//! can be two dozen rows. On the shipped benchmarks that is 759 rows for 484
//! contacts.
//!
//! Decided 2026-08-23, after the question sat open for four runs: **the rule
//! keeps counting pairs of segments, and the contact count is published beside
//! it.** A violation is a place, and two segments of one trace touching a pad
//! at two points are two places a fabricator's etch can fail - collapsing them
//! in the model loses the locations. The counts are also regression ratchets,
//! and a per-contact count is the coarser number: it would mask a routing
//! change that a per-segment count catches. What a reader outside this project
//! needed was never a different count but a second one.

use cypcb_core::{Nm, Point};
use cypcb_drc::{clearance_contacts, pair_of, DrcViolation};
use cypcb_world::Entity;

fn clearance(pair: &str, actual_mm: f64) -> DrcViolation {
    let mut violation = DrcViolation::clearance(
        Entity::from_raw(1),
        Entity::from_raw(2),
        Nm::from_mm(actual_mm),
        Nm::from_mm(0.15),
        Point::from_mm(1.0, 1.0),
    );
    // The entity labels the rule prepends when it knows what the two features
    // are called - which is what makes a contact identifiable at all.
    violation.message = format!("{pair}: {}", violation.message);
    violation
}

#[test]
fn a_run_of_segments_against_one_pad_is_one_contact() {
    let rows = vec![
        clearance("U1.3 <-> trace 'GND'", 0.09),
        clearance("U1.3 <-> trace 'GND'", 0.04),
        clearance("U1.3 <-> trace 'GND'", 0.11),
    ];
    assert_eq!(rows.len(), 3, "the premise: three rows");
    assert_eq!(clearance_contacts(&rows), 1);
}

#[test]
fn two_different_pairs_are_two_contacts() {
    let rows = vec![
        clearance("U1.3 <-> trace 'GND'", 0.04),
        clearance("R7.1 <-> trace 'VCC'", 0.04),
    ];
    assert_eq!(clearance_contacts(&rows), 2);
}

#[test]
fn nothing_but_clearance_is_counted() {
    // The other kinds report per feature, and two of their messages being
    // equal is two faults rather than one seen twice - so they are not
    // contacts and must not be counted as any.
    let mut rows = vec![clearance("U1.3 <-> trace 'GND'", 0.04)];
    rows.push(DrcViolation::unconnected_pin(
        Entity::from_raw(3),
        "1",
        "R1",
        Point::from_mm(2.0, 2.0),
    ));
    assert_eq!(clearance_contacts(&rows), 1);
}

#[test]
fn a_board_with_no_clearance_faults_has_no_contacts() {
    assert_eq!(clearance_contacts(&[]), 0);
}

#[test]
fn the_pair_is_everything_before_the_first_colon() {
    assert_eq!(
        pair_of("U1 <-> trace 'GND': Clearance violation: 0.05mm actual"),
        "U1 <-> trace 'GND'"
    );
    assert_eq!(pair_of("drill too small"), "drill too small");
}
