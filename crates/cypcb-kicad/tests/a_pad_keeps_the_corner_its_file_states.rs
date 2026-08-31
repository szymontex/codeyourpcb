//! A pad keeps the corner its file states.
//!
//! KiCad writes `(roundrect_rratio ...)` inside a rounded pad, and this crate
//! read 25% for every one of them whatever the file said. Measured across the
//! KiCad files in this repository: 427 pads state 0.25 and four state 0.2, so
//! four pads on a real board came back with corners a fifth larger than the
//! ones it was drawn with. A pad's corner is the copper nearest its neighbour,
//! which is what a clearance is measured from.

use cypcb_world::components::PadShape;

fn only_pad(source: &str) -> PadShape {
    let footprint = cypcb_kicad::import_footprint_from_str(source)
        .unwrap_or_else(|error| panic!("the fixture does not read: {error}"));
    assert_eq!(footprint.pads.len(), 1, "the fixture states one pad");
    footprint.pads[0].shape
}

fn pad_with(ratio: &str) -> String {
    format!(
        r#"(footprint "Fixture"
  (version 20240108)
  (generator "pcbnew")
  (layer "F.Cu")
  (pad "1" smd roundrect (at 0 0) (size 1 1) (layers "F.Cu"){ratio})
)"#
    )
}

#[test]
fn the_ratio_the_file_states_is_the_ratio_the_pad_carries() {
    assert_eq!(
        only_pad(&pad_with(" (roundrect_rratio 0.25)")),
        PadShape::RoundRect { corner_ratio: 25 }
    );
    // The one this repository's own boards state four times, and the reading
    // the old code could not produce.
    assert_eq!(
        only_pad(&pad_with(" (roundrect_rratio 0.2)")),
        PadShape::RoundRect { corner_ratio: 20 }
    );
}

#[test]
fn a_pad_that_states_no_ratio_keeps_the_fallback() {
    // Nothing in the file says 25: it is what this reader has always used, and
    // the test is here so that stays a decision rather than a leftover.
    assert_eq!(
        only_pad(&pad_with("")),
        PadShape::RoundRect { corner_ratio: 25 }
    );
}

#[test]
fn a_ratio_beyond_half_the_pad_is_held_to_half() {
    // A ratio is of the short side, so half of it is a stadium and there is
    // nothing past that to draw.
    assert_eq!(
        only_pad(&pad_with(" (roundrect_rratio 0.9)")),
        PadShape::RoundRect { corner_ratio: 50 }
    );
}

#[test]
fn a_real_footprint_kicad_wrote_carries_its_own_corners() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../viewer/kicad-tools/tests/fixtures/Test_Library.pretty/SOT-23-5.kicad_mod");
    let footprint = cypcb_kicad::import_footprint(&file).expect("the fixture reads");
    assert_eq!(footprint.pads.len(), 5);
    for pad in &footprint.pads {
        assert_eq!(
            pad.shape,
            PadShape::RoundRect { corner_ratio: 25 },
            "pad {} states 0.25",
            pad.number
        );
    }
}
