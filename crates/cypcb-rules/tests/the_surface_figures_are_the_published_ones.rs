//! The surface figures are the ones the fabs publish, in the units they use.
//!
//! `cargo test -p cypcb-rules --test the_surface_figures_are_the_published_ones`
//!
//! Copper was read against every fab's own page over several passes. The
//! silkscreen, mask and paste column was not read for any preset until
//! 2026-08-21, and the first house checked had three of four figures wrong.
//!
//! Two of the mistakes are worth naming because they are the kind a table
//! grows on its own:
//!
//! - **A mil figure rounded to two decimals.** 4 mil is 0.1016mm, and both
//!   OSH Park and PCBWay carried `0.1` under a `// 4 mil` comment - looser
//!   than the page by 1.6 microns, in the fab's own name.
//! - **A margin written as a capability.** PCBWay's legend read 0.22mm and
//!   its edge clearance 0.3mm; the page publishes 0.15mm and 0.25mm. A
//!   capability table says what a house can make, and a margin on top of it
//!   is the designer's to choose.

use cypcb_core::Nm;
use cypcb_rules::presets::RulesPreset;

/// A mil in nanometres, so a figure a page states in mils is compared against
/// the arithmetic rather than against somebody's rounding of it.
const MIL: f64 = 0.0254;

fn mils(count: f64) -> Nm {
    Nm::from_mm(count * MIL)
}

#[test]
fn the_mask_web_is_four_mils_not_a_tenth_of_a_millimetre() {
    // OSH Park publishes "4 mil (0.1016mm)" as the minimum soldermask web on
    // both service pages; PCBWay publishes 4 mil for copper under 2oz.
    for preset in [
        RulesPreset::OshPark2Layer,
        RulesPreset::OshPark4Layer,
        RulesPreset::PcbWayStandard,
    ] {
        assert_eq!(
            preset.constraints().min_solder_mask_bridge,
            mils(4.0),
            "{preset:?} does not carry the published 4 mil"
        );
    }
    assert_eq!(mils(4.0), Nm::from_mm(0.1016), "4 mil is 0.1016mm");
}

#[test]
fn pcbway_carries_the_page_rather_than_a_margin_on_top_of_it() {
    let pcbway = RulesPreset::PcbWayStandard.constraints();
    // Published: 0.15mm minimum legend width, 0.25mm line-to-board-edge for
    // the standard CNC-milled process, 2 mil standard mask opening.
    assert_eq!(pcbway.min_silk_width, Nm::from_mm(0.15));
    assert_eq!(pcbway.min_edge_clearance, Nm::from_mm(0.25));
    assert_eq!(pcbway.solder_mask_expansion, mils(2.0));
    assert_eq!(mils(2.0), Nm::from_mm(0.0508), "2 mil is 0.0508mm");
}

#[test]
fn osh_park_carries_the_two_figures_its_pages_do_state() {
    // 5 mil recommended minimum legend, 15 mil board edge keepout, both
    // service pages, both identical.
    for preset in [RulesPreset::OshPark2Layer, RulesPreset::OshPark4Layer] {
        let rules = preset.constraints();
        assert_eq!(rules.min_silk_width, mils(5.0), "{preset:?}");
        assert_eq!(rules.min_edge_clearance, mils(15.0), "{preset:?}");
    }
}

#[test]
fn jlcpcb_carries_the_one_legend_and_dam_its_page_states() {
    // One published figure each, on every layer count and every tier - the
    // page has no tier under it, which is the finding that came with this
    // read. 0.15mm legend, 0.10mm minimum pad spacing for a mask dam.
    for preset in [
        RulesPreset::JlcpcbStandard2Layer,
        RulesPreset::JlcpcbStandard4Layer,
        RulesPreset::JlcpcbAdvanced2Layer,
        RulesPreset::JlcpcbAdvanced4Layer,
    ] {
        let rules = preset.constraints();
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.15), "{preset:?}");
        assert_eq!(rules.min_solder_mask_bridge, Nm::from_mm(0.1), "{preset:?}");
    }
}

#[test]
fn the_paste_aperture_is_the_same_unsourced_number_everywhere() {
    // Neither OSH Park nor PCBWay publishes a stencil aperture at all, and
    // this is the guard on that: three houses carrying one identical figure
    // is a default, not three fabs agreeing. If one of them ever gets a real
    // number, this fails and the comment beside it has to be written.
    let figures: Vec<Nm> = [
        RulesPreset::OshPark2Layer,
        RulesPreset::OshPark4Layer,
        RulesPreset::PcbWayStandard,
    ]
    .into_iter()
    .map(|preset| preset.constraints().min_paste_clearance)
    .collect();
    assert!(
        figures.iter().all(|figure| *figure == figures[0]),
        "one of these is sourced now and this test has not been told: {figures:?}"
    );
}
