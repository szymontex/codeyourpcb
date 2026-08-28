//! Every tolerance in the fab tables is one a house published.
//!
//! `cargo test -p cypcb-rules --test the_tolerances_are_the_published_ones`
//!
//! Three numbers in this project describe how far off a finished board may
//! come out: the thickness, the thin-board figure that replaces it, and the
//! hole. All three belong to the fabricator rather than to the design - a
//! house publishes them and a board is held to them - so the table has to say
//! exactly what was read, and `None` where nothing was.
//!
//! What each entry is quoted from is in the comment beside it in the preset.
//! This test is the other half: it fails when a figure changes without the
//! reading changing, and when a house is given a figure it never published.

use cypcb_core::Nm;
use cypcb_rules::presets::RulesPreset;

fn table(name: &str) -> cypcb_rules::DesignConstraints {
    RulesPreset::from_name(name)
        .unwrap_or_else(|| panic!("{name} is a preset"))
        .constraints()
}

#[test]
fn jlcpcb_states_two_thickness_rules_and_an_asymmetric_hole() {
    // "± 10%" at 1.0mm and above, "± 0.1mm" below it, and through-holes at
    // "+0.13 / -0.08 mm" - plating grows into the barrel, so the two ends of
    // the hole figure are different numbers.
    let jlcpcb = table("jlcpcb");
    assert_eq!(jlcpcb.board_thickness_tolerance_percent, Some(10));
    assert_eq!(
        jlcpcb.board_thickness_tolerance_thin,
        Some(Nm::from_mm(0.1))
    );
    assert_eq!(jlcpcb.hole_tolerance_plus, Some(Nm::from_mm(0.13)));
    assert_eq!(jlcpcb.hole_tolerance_minus, Some(Nm::from_mm(0.08)));
    assert_ne!(
        jlcpcb.hole_tolerance_plus, jlcpcb.hole_tolerance_minus,
        "the asymmetry is the fact, not an accident of transcription"
    );
}

#[test]
fn pcbway_states_the_same_thickness_rules_and_a_symmetric_hole() {
    let pcbway = table("pcbway");
    assert_eq!(pcbway.board_thickness_tolerance_percent, Some(10));
    assert_eq!(
        pcbway.board_thickness_tolerance_thin,
        Some(Nm::from_mm(0.1))
    );
    assert_eq!(pcbway.hole_tolerance_plus, Some(Nm::from_mm(0.08)));
    assert_eq!(pcbway.hole_tolerance_minus, Some(Nm::from_mm(0.08)));
}

#[test]
fn oshpark_publishes_a_hole_figure_and_no_thickness_one() {
    // The service page states the thickness as "63mil (1.6mm) nominal" and
    // stops there. A tolerance filled in for it would be a promise nobody
    // made; the drill figure it does publish is carried at its maximum,
    // because the maximum is what a board is guaranteed.
    let oshpark = table("oshpark");
    assert_eq!(
        oshpark.board_thickness_tolerance_percent, None,
        "no thickness tolerance is published, so none is carried"
    );
    assert_eq!(oshpark.board_thickness_tolerance_thin, None);
    assert_eq!(oshpark.hole_tolerance_plus, Some(Nm::from_mm(0.0635)));
    assert_eq!(oshpark.hole_tolerance_minus, Some(Nm::from_mm(0.0635)));
}

#[test]
fn the_ipc_tables_claim_no_tolerance_at_all() {
    // IPC-6012 states these per performance class and the standard is
    // paywalled. Nothing here has read it, so nothing is claimed - the same
    // rule this project's IPC tables already follow for clauses they cannot
    // show.
    for name in ["ipc1", "ipc2", "ipc3"] {
        let Some(preset) = RulesPreset::from_name(name) else {
            continue;
        };
        let table = preset.constraints();
        assert_eq!(table.board_thickness_tolerance_percent, None, "{name}");
        assert_eq!(table.board_thickness_tolerance_thin, None, "{name}");
        assert_eq!(table.hole_tolerance_plus, None, "{name}");
        assert_eq!(table.hole_tolerance_minus, None, "{name}");
    }
}
