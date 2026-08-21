//! Two impedances, worked out by hand before the code was run.
//!
//! `cargo test -p cypcb-calc --test the_impedance_is_the_published_closed_form`
//!
//! The anchors below are the point of this file. Each was computed on paper
//! from IPC-2141's own equation and written down **before** the implementation
//! was executed, so the test is not the code agreeing with itself. The
//! arithmetic is in the comment beside each one, in the order a reader can
//! follow with a calculator.
//!
//! What this file does not do is check either equation against a third party.
//! Nothing available to this project does: IPC-2141 is behind a paywall, the
//! tutorial with worked examples would not load, one calculator page renders
//! its formula as an image, and KiCad's own microstrip calculator has an open
//! issue saying it may be more than 20% out. The module says so at the top and
//! so does the report that will eventually carry the number.

use cypcb_calc::{microstrip_ohms_x100, stripline_ohms_x100};
use cypcb_core::Nm;

fn mm(value: f64) -> Nm {
    Nm::from_mm(value)
}

#[test]
fn a_microstrip_on_fr4_comes_out_where_the_arithmetic_says() {
    // W 0.35mm, H 0.2mm, T 0.035mm (1oz), Er 4.3.
    //
    //   0.8W + T   = 0.28 + 0.035        = 0.315
    //   5.98H      = 5.98 * 0.2          = 1.196
    //   ratio      = 1.196 / 0.315       = 3.796825
    //   ln(ratio)                        = 1.334165
    //   sqrt(Er + 1.41) = sqrt(5.71)     = 2.389561
    //   87 / 2.389561                    = 36.40791
    //   Z0 = 36.40791 * 1.334165         = 48.573 ohm
    assert_eq!(
        microstrip_ohms_x100(mm(0.35), mm(0.2), mm(0.035), 4_300),
        Some(4857)
    );
}

#[test]
fn a_stripline_comes_out_where_the_arithmetic_says() {
    // W 0.2mm, B 0.4mm, T 0.035mm, Er 4.5.
    //
    //   0.8W + T      = 0.16 + 0.035     = 0.195
    //   0.67 * pi                        = 2.104867
    //   denominator   = 2.104867 * 0.195 = 0.410449
    //   4B            = 1.6
    //   ratio         = 1.6 / 0.410449   = 3.898182
    //   ln(ratio)                        = 1.360510
    //   60 / sqrt(4.5) = 60 / 2.121320   = 28.284271
    //   Z0 = 28.284271 * 1.360510        = 38.481 ohm
    assert_eq!(
        stripline_ohms_x100(mm(0.2), mm(0.4), mm(0.035), 4_500),
        Some(3848)
    );
}

#[test]
fn the_impedance_moves_the_way_the_physics_does() {
    // Three directions, each with one variable moving. A transcription error
    // in a constant usually survives one anchor and does not survive these.
    let base = microstrip_ohms_x100(mm(0.35), mm(0.2), mm(0.035), 4_300).expect("the base case");

    let wider = microstrip_ohms_x100(mm(0.5), mm(0.2), mm(0.035), 4_300).expect("wider");
    assert!(wider < base, "a wider trace is a lower impedance: {wider}");

    let taller = microstrip_ohms_x100(mm(0.35), mm(0.3), mm(0.035), 4_300).expect("taller");
    assert!(
        taller > base,
        "further from its plane is a higher impedance: {taller}"
    );

    let denser = microstrip_ohms_x100(mm(0.35), mm(0.2), mm(0.035), 6_000).expect("denser");
    assert!(
        denser < base,
        "a denser dielectric is a lower impedance: {denser}"
    );

    let stripline = stripline_ohms_x100(mm(0.2), mm(0.4), mm(0.035), 4_500).expect("the base case");
    let stripline_wider = stripline_ohms_x100(mm(0.3), mm(0.4), mm(0.035), 4_500).expect("wider");
    assert!(stripline_wider < stripline);
}

#[test]
fn a_stripline_is_the_tighter_of_the_two_at_the_same_geometry() {
    // Copper buried between two planes couples to both, so it carries a lower
    // impedance than the same trace over one. Treating a stripline with the
    // microstrip form - or the reverse - is the mistake this guards, and it is
    // the one a user makes by choosing the wrong function rather than by
    // typing the wrong number.
    let over_one_plane = microstrip_ohms_x100(mm(0.2), mm(0.4), mm(0.035), 4_500).expect("micro");
    let between_two = stripline_ohms_x100(mm(0.2), mm(0.4), mm(0.035), 4_500).expect("strip");
    assert!(
        between_two < over_one_plane,
        "stripline {between_two} should be under microstrip {over_one_plane}"
    );
}

#[test]
fn what_the_form_cannot_answer_comes_back_as_nothing() {
    // A number outside the range these equations are quoted for is not a small
    // error, it is a meaningless one - and a meaningless impedance printed to
    // two decimals reads exactly like a measurement.
    assert_eq!(
        microstrip_ohms_x100(mm(0.35), mm(0.2), mm(0.035), 1_000),
        None,
        "Er of 1.0 is vacuum, and the range is quoted as 1 < Er < 15"
    );
    assert_eq!(
        microstrip_ohms_x100(mm(0.35), mm(0.2), mm(0.035), 15_000),
        None,
        "and 15 is the other end of it"
    );
    assert_eq!(
        microstrip_ohms_x100(mm(0.0), mm(0.2), mm(0.035), 4_300),
        None,
        "a trace with no width is not a trace"
    );
    assert_eq!(
        microstrip_ohms_x100(mm(0.35), mm(0.0), mm(0.035), 4_300),
        None,
        "nor is copper lying on its own reference plane"
    );
    // A trace so wide that the logarithm's argument falls to one gives zero
    // ohms, and below one it gives a negative number.
    assert_eq!(
        microstrip_ohms_x100(mm(2.0), mm(0.2), mm(0.035), 4_300),
        None,
        "the form has run out before this geometry"
    );
    assert_eq!(
        stripline_ohms_x100(mm(0.2), mm(0.4), mm(0.035), 20_000),
        None
    );
}
