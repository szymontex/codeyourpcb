//! The four closed forms, held to their own arithmetic.
//!
//! `cargo test -p cypcb-calc --test the_forms_answer_each_other`
//!
//! `cypcb-calc` is where IPC-2141's impedance equations live, and every
//! impedance figure this project prints comes out of these four functions.
//! They were exercised only through the checker's messages: counting the
//! crate's public functions found all four used elsewhere, named in
//! integration tests, and with no test of their own.
//!
//! What is checked here is what can be checked without a table nobody in this
//! repository can read. The forms answer each other: ask what a width gives,
//! then ask which width gives that, and the answer has to be the width you
//! started with. A coefficient typed wrong in either direction breaks the
//! round trip, and the two directions are separate code - one is the equation,
//! the other is a bisection over it.
//!
//! The monotonic direction is checked too, because it is the half a round trip
//! cannot see: a wider trace over the same dielectric is a lower impedance,
//! and a thicker dielectric under the same trace is a higher one.

use cypcb_calc::{
    microstrip_ohms_x100, microstrip_width_for_ohms_x100, stripline_ohms_x100,
    stripline_width_for_ohms_x100,
};
use cypcb_core::Nm;

/// An ordinary outer-layer geometry: 0.2mm of trace over 0.2mm of FR4.
const HEIGHT: Nm = Nm(200_000);
const COPPER: Nm = Nm(35_000);
const DK: u32 = 4_500;

#[test]
fn a_microstrip_width_comes_back_from_the_impedance_it_gives() {
    for width_um in [100u32, 150, 200, 300, 500] {
        let width = Nm(i64::from(width_um) * 1_000);
        let ohms = microstrip_ohms_x100(width, HEIGHT, COPPER, DK)
            .unwrap_or_else(|| panic!("{width_um}um over 0.2mm is a microstrip this form covers"));

        let back = microstrip_width_for_ohms_x100(ohms, HEIGHT, COPPER, DK)
            .unwrap_or_else(|| panic!("{ohms} hundredths of an ohm is reachable at some width"));

        // The bisection stops when it is close enough for a fabricator, which
        // is a micron rather than a nanometre.
        let off_by = (back.raw() - width.raw()).abs();
        assert!(
            off_by <= 2_000,
            "{width_um}um gives {ohms} and that asks for {}nm back, {off_by}nm away",
            back.raw()
        );
    }
}

#[test]
fn a_stripline_width_comes_back_too() {
    // A buried trace, centred between two planes 0.4mm apart.
    let separation = Nm(400_000);
    for width_um in [100u32, 150, 200, 300] {
        let width = Nm(i64::from(width_um) * 1_000);
        let ohms = stripline_ohms_x100(width, separation, COPPER, DK)
            .unwrap_or_else(|| panic!("{width_um}um between planes is a stripline"));
        let back = stripline_width_for_ohms_x100(ohms, separation, COPPER, DK)
            .unwrap_or_else(|| panic!("{ohms} is reachable"));

        let off_by = (back.raw() - width.raw()).abs();
        assert!(
            off_by <= 2_000,
            "{width_um}um gives {ohms} and that asks for {}nm back, {off_by}nm away",
            back.raw()
        );
    }
}

#[test]
fn wider_copper_is_a_lower_impedance() {
    let mut last = u32::MAX;
    for width_um in [100u32, 150, 200, 300, 500, 800] {
        let ohms = microstrip_ohms_x100(Nm(i64::from(width_um) * 1_000), HEIGHT, COPPER, DK)
            .expect("a width this form covers");
        assert!(
            ohms < last,
            "{width_um}um reads {ohms} where the narrower trace read {last}: a \
             wider trace over the same dielectric is a lower impedance"
        );
        last = ohms;
    }
}

#[test]
fn a_thicker_dielectric_is_a_higher_impedance() {
    let mut last = 0;
    for height_um in [100u32, 200, 400, 800] {
        let ohms = microstrip_ohms_x100(Nm(200_000), Nm(i64::from(height_um) * 1_000), COPPER, DK)
            .expect("a height this form covers");
        assert!(
            ohms > last,
            "{height_um}um of laminate reads {ohms} where the thinner stack read \
             {last}: the further the trace is from its plane, the higher the \
             impedance"
        );
        last = ohms;
    }
}

#[test]
fn one_geometry_worked_by_hand() {
    // The half a round trip cannot see: both directions run the same equation,
    // so a coefficient typed wrong comes back consistent with itself. This is
    // IPC-2141's microstrip form written out for one geometry - 0.2mm of trace
    // and 1oz of copper over 0.2mm of dk 4.5 laminate:
    //
    //   effective width = 0.8 x 0.2 + 0.035           = 0.195mm
    //   ratio           = 5.98 x 0.2 / 0.195          = 6.1333
    //   ln(ratio)                                     = 1.81374
    //   87 / sqrt(4.5 + 1.41)                         = 35.787
    //   Z               = 35.787 x 1.81374            = 64.91 ohm
    //
    // The same board through `cypcb check` reports `gives 64.91ohm`.
    let ohms = microstrip_ohms_x100(Nm(200_000), Nm(200_000), Nm(35_000), 4_500)
        .expect("an ordinary outer-layer geometry");
    assert_eq!(
        ohms, 6491,
        "the form is 87/sqrt(er+1.41) x ln(5.98h/(0.8w+t)), and for this stack \
         that is 64.91 ohm"
    );
}

#[test]
fn a_stack_with_no_thickness_has_no_answer() {
    // The half that keeps the forms from inventing numbers: a zero-height
    // dielectric is not a microstrip, and a zero dk is not a material.
    assert_eq!(microstrip_ohms_x100(Nm(200_000), Nm(0), COPPER, DK), None);
    assert_eq!(microstrip_ohms_x100(Nm(200_000), HEIGHT, COPPER, 0), None);
}
