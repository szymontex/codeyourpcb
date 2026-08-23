//! The width a target impedance asks for.
//!
//! `cargo test -p cypcb-calc --test the_width_a_target_impedance_asks_for`
//!
//! The forward direction has been here since the impedance rule was written:
//! a width goes in, an impedance comes out. That is not the question a
//! designer has. The stack is what the fabricator presses and the target is
//! what the part datasheet demands, so the width is the only thing left to
//! choose - and Altium's stack manager has answered it for twenty years while
//! this project answered "you are 55.4% off" and left the arithmetic to the
//! reader.
//!
//! Neither closed form inverts: the width sits inside a logarithm and under a
//! correction for the foil thickness. So the answer is searched for, which is
//! what every field solver and every fab calculator does with these same
//! equations.

use cypcb_calc::{
    microstrip_ohms_x100, microstrip_width_for_ohms_x100, stripline_ohms_x100,
    stripline_width_for_ohms_x100,
};
use cypcb_core::Nm;

/// An ordinary outer layer: 0.2mm of FR4 under 1oz copper.
fn microstrip(target_x100: u32) -> Option<Nm> {
    microstrip_width_for_ohms_x100(target_x100, Nm::from_mm(0.2), Nm::from_mm(0.035), 4_500)
}

/// An ordinary inner layer: 0.4mm between the planes, half-ounce copper.
fn stripline(target_x100: u32) -> Option<Nm> {
    stripline_width_for_ohms_x100(target_x100, Nm::from_mm(0.4), Nm::from_mm(0.0175), 4_500)
}

#[test]
fn the_width_it_returns_gives_the_impedance_it_was_asked_for() {
    // The whole promise, checked against the forward form rather than against
    // a number written down here: a solver that agrees only with itself has
    // proved nothing.
    // 90 ohm and under: reachable on both of these stacks. The ceilings are
    // measured in `the_ceiling_is_whatever_the_narrowest_trace_gives` below.
    for target in [4_000, 5_000, 7_500, 9_000] {
        let width = microstrip(target).unwrap_or_else(|| panic!("{target} is reachable"));
        let back = microstrip_ohms_x100(width, Nm::from_mm(0.2), Nm::from_mm(0.035), 4_500)
            .expect("in range");
        assert!(
            back.abs_diff(target) <= 2,
            "microstrip {target}: solved {width:?} which gives {back}"
        );

        let width = stripline(target).unwrap_or_else(|| panic!("{target} is reachable"));
        let back = stripline_ohms_x100(width, Nm::from_mm(0.4), Nm::from_mm(0.0175), 4_500)
            .expect("in range");
        assert!(
            back.abs_diff(target) <= 2,
            "stripline {target}: solved {width:?} which gives {back}"
        );
    }
}

#[test]
fn a_higher_target_is_a_narrower_trace() {
    // Both forms are `k * ln(c / w)`: monotone decreasing in width. If this
    // ever reverses, the search is walking the wrong way and the round trip
    // above would still pass on whichever end it happened to land on.
    let fifty = microstrip(5_000).expect("reachable");
    let ninety = microstrip(9_000).expect("reachable");
    assert!(ninety.raw() < fifty.raw(), "{ninety:?} vs {fifty:?}");

    let fifty = stripline(5_000).expect("reachable");
    let ninety = stripline(9_000).expect("reachable");
    assert!(ninety.raw() < fifty.raw(), "{ninety:?} vs {fifty:?}");
}

#[test]
fn a_stripline_wants_a_narrower_trace_than_a_microstrip() {
    // Copper between two planes couples to both, so the same width reads lower
    // and the same target needs less of it. This is the check that would catch
    // the two solvers being wired to each other's form.
    let strip = stripline(5_000).expect("reachable");
    let micro = microstrip(5_000).expect("reachable");
    assert!(strip.raw() < micro.raw(), "{strip:?} vs {micro:?}");
}

#[test]
fn a_target_this_stack_cannot_deliver_is_refused() {
    // 1 ohm on 0.2mm of FR4 would need a trace wider than most boards. Naming
    // a width nobody can etch is worse than saying nothing, which is what the
    // rule above this prints when the answer is `None`.
    assert_eq!(microstrip(100), None);
    assert_eq!(stripline(100), None);
}

#[test]
fn a_target_above_what_the_narrowest_trace_gives_is_refused() {
    // The other end. 300 ohm is not reachable at any width a fabricator
    // images, on either of these stacks.
    assert_eq!(microstrip(30_000), None);
    assert_eq!(stripline(30_000), None);
}

#[test]
fn the_ceiling_is_whatever_the_narrowest_trace_gives() {
    // Measured on these two stacks at 0.01mm, which is the narrowest width the
    // search looks at: the microstrip reads **119.01 ohm** and the stripline
    // **96.02 ohm**. So 97 ohm is an answer on one and not on the other, and
    // that difference is the whole reason the ceiling is a measurement rather
    // than a constant somebody picked.
    assert!(
        microstrip(9_700).is_some(),
        "119.01 ohm is the ceiling here"
    );
    assert_eq!(stripline(9_700), None, "96.02 ohm is the ceiling here");
}

#[test]
fn a_target_of_nothing_is_refused() {
    assert_eq!(microstrip(0), None);
    assert_eq!(stripline(0), None);
}

#[test]
fn a_stack_the_form_refuses_has_no_width_either() {
    // No dielectric constant is no answer, in both directions: the forward
    // form refuses it and so must the search, rather than bisecting on a
    // function that answers `None` everywhere.
    assert_eq!(
        microstrip_width_for_ohms_x100(5_000, Nm::from_mm(0.2), Nm::from_mm(0.035), 0),
        None
    );
    assert_eq!(
        stripline_width_for_ohms_x100(5_000, Nm(0), Nm::from_mm(0.0175), 4_500),
        None
    );
}
