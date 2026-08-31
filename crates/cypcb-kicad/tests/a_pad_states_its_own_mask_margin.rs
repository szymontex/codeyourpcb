//! A pad states its own solder mask opening, and the reader keeps it.
//!
//! `cargo test -p cypcb-kicad --test a_pad_states_its_own_mask_margin`
//!
//! The mask opening runs past the copper by one figure from the fabricator's
//! table, which is right for nearly every pad on a board. A pad states its own
//! when the part needs one: KiCad writes `(solder_mask_margin 0.1016)`, which
//! is 4 mil, inside a through-hole connector's pads so the mask does not creep
//! onto copper a hand-soldered joint has to wet.
//!
//! Measured 2026-08-31 across the KiCad files in this repository: 124 pads of
//! 2623 state one, all of them in the footprint library under
//! `viewer/svg-pcb/kicad-components`, and every one was dropped at import.

use cypcb_core::Nm;

#[test]
fn a_footprint_kicad_wrote_carries_the_margin_it_states() {
    // `fab-1X04.kicad_mod` is a four-pin header; each of its pads states
    // 0.1016.
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../viewer/svg-pcb/kicad-components/fab-1X04.kicad_mod");
    let footprint = cypcb_kicad::import_footprint(&file).expect("the fixture reads");

    assert_eq!(footprint.pads.len(), 4);
    for pad in &footprint.pads {
        assert_eq!(
            pad.mask_margin,
            Some(Nm::from_mm(0.1016)),
            "pad {} states its own mask margin",
            pad.number
        );
    }
}

#[test]
fn a_pad_that_states_none_asks_for_nothing() {
    // Every other footprint in this repository: the board's figure covers it,
    // and `None` is what says so. A zero here would be a pad asking for an
    // opening the size of its own copper, which is a different board.
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../viewer/kicad-tools/tests/fixtures/Test_Library.pretty/SOT-23-5.kicad_mod");
    let footprint = cypcb_kicad::import_footprint(&file).expect("the fixture reads");

    assert_eq!(footprint.pads.len(), 5);
    for pad in &footprint.pads {
        assert_eq!(pad.mask_margin, None, "pad {} states no margin", pad.number);
    }
}
