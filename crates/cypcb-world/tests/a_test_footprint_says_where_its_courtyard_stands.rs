//! A footprint a test builds is held to its courtyard, or says why not.
//!
//! `cargo test -p cypcb-world --test a_test_footprint_says_where_its_courtyard_stands`
//!
//! The tests of this repository define footprints by hand, and the corpus
//! that checks every footprint in a file cannot see them. Three calls end such
//! a definition, and these are what each one does:
//!
//! - `expect_pads_in_courtyard` stops the test that builds a footprint whose
//!   copper leaves its courtyard;
//! - `pads_outside_courtyard_because` is for the footprint that is wrong on
//!   purpose, and stops the test when it is not wrong any more;
//! - `with_courtyard_holding_its_pads` widens a courtyard by what its copper
//!   needs and leaves one that already holds it exactly as drawn.
//!
//! The footprints here are the built-in 0402 with its courtyard replaced, so
//! the file defines no `Footprint { .. }` of its own.

use cypcb_core::{Nm, Point, Rect};
use cypcb_world::components::Rotation;
use cypcb_world::footprint::{Footprint, FootprintLibrary};

/// The built-in 0402 - pads at x = -0.5 and 0.5, each 0.6 by 0.6mm, copper
/// spanning x -0.8..0.8 and y -0.3..0.3 - with its courtyard replaced by a box
/// `width` by `height` mm about the origin.
fn chip(width_mm: f64, height_mm: f64) -> Footprint {
    let mut footprint = FootprintLibrary::new()
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    footprint.courtyard = Rect::from_center_size(
        Point::ORIGIN,
        (Nm::from_mm(width_mm), Nm::from_mm(height_mm)),
    );
    footprint
}

#[test]
fn a_courtyard_that_holds_the_pads_passes_through_expect_unchanged() {
    let held = chip(2.0, 1.0);
    let before = held.courtyard;
    let after = held.expect_pads_in_courtyard();
    assert_eq!(after.courtyard, before);
    assert_eq!(after.pads.len(), 2);
}

#[test]
#[should_panic(expected = "has pads outside its courtyard: pad 1 by 100000 nm, pad 2 by 100000 nm")]
fn a_courtyard_that_does_not_hold_the_pads_stops_expect_naming_each_pad() {
    // 1.4mm wide reaches 0.7 where the copper reaches 0.8.
    chip(1.4, 1.0).expect_pads_in_courtyard();
}

#[test]
fn a_footprint_wrong_on_purpose_passes_through_with_its_reason() {
    let short = chip(1.4, 1.0);
    let before = short.courtyard;
    let after = short.pads_outside_courtyard_because("the rule under test reports it");
    assert_eq!(after.courtyard, before, "it is left as wrong as it was");
}

#[test]
#[should_panic(expected = "an exemption without a reason")]
fn an_exemption_without_a_reason_is_refused() {
    chip(1.4, 1.0).pads_outside_courtyard_because("   ");
}

#[test]
#[should_panic(expected = "but every pad is inside its courtyard now")]
fn an_exemption_for_a_footprint_that_is_right_now_is_refused() {
    chip(2.0, 1.0).pads_outside_courtyard_because("it was short once");
}

#[test]
fn a_courtyard_that_holds_its_pads_is_left_exactly_as_drawn() {
    let held = chip(2.0, 1.0);
    let before = held.courtyard;
    assert_eq!(held.with_courtyard_holding_its_pads().courtyard, before);
}

#[test]
fn a_courtyard_that_is_short_grows_by_what_the_copper_needs_and_no_more() {
    // 1.4mm wide, 1.0 tall: short by 0.1mm each side along x, enough along y.
    let grown = chip(1.4, 1.0).with_courtyard_holding_its_pads();
    assert_eq!(grown.courtyard.min, Point::new(Nm(-800_000), Nm(-500_000)));
    assert_eq!(grown.courtyard.max, Point::new(Nm(800_000), Nm(500_000)));
    assert!(grown.pads_outside_courtyard().is_empty());
}

#[test]
fn a_courtyard_that_is_short_on_one_side_grows_on_that_side_only() {
    let mut off = chip(2.0, 1.0);
    off.courtyard = Rect::new(
        Point::new(Nm(-1_000_000), Nm(-500_000)),
        Point::new(Nm(500_000), Nm(500_000)),
    );
    let grown = off.with_courtyard_holding_its_pads();
    assert_eq!(
        grown.courtyard.min,
        Point::new(Nm(-1_000_000), Nm(-500_000))
    );
    assert_eq!(grown.courtyard.max, Point::new(Nm(800_000), Nm(500_000)));
}

#[test]
fn a_pad_stood_up_is_grown_to_by_the_sides_it_stands_on() {
    // Each pad turned a quarter: 0.6 wide by 0.3 tall becomes 0.3 by 0.6 on
    // the board, so the courtyard has to reach 0.3 along y, not 0.15.
    let mut up = chip(2.0, 1.0);
    for pad in &mut up.pads {
        pad.size = (Nm(600_000), Nm(300_000));
        pad.rotation = Rotation::DEG_90;
    }
    up.courtyard = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.0), Nm::from_mm(0.4)));
    assert_eq!(
        up.pads_outside_courtyard().len(),
        2,
        "short along y once stood up"
    );
    let grown = up.with_courtyard_holding_its_pads();
    assert_eq!(grown.courtyard.max.y, Nm(300_000));
    assert_eq!(grown.courtyard.min.y, Nm(-300_000));
}

#[test]
fn a_footprint_with_no_pads_is_left_alone_by_all_three() {
    let mut bare = chip(2.0, 1.0);
    bare.pads.clear();
    let before = bare.courtyard;
    assert_eq!(bare.clone().expect_pads_in_courtyard().courtyard, before);
    assert_eq!(
        bare.clone().with_courtyard_holding_its_pads().courtyard,
        before
    );
}
