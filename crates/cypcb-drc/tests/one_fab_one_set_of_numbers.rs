//! The two preset tables have to say the same thing about the same fab.
//!
//! `cargo test -p cypcb-drc --test one_fab_one_set_of_numbers`
//!
//! This project carries **two** preset registries. `cypcb_drc::Preset` is what
//! the checker and every CLI command resolve `--preset` through; the router is
//! priced from `cypcb_rules::RulesPreset`. They were written separately, they
//! disagree on which fabs exist - the checker has `prototype` and no IPC
//! classes, the rules crate documents `ipc1`, `ipc2` and `ipc3` and every
//! command refuses them - and they even spell the same fab differently
//! (`jlcpcb_2layer` against `jlcpcb_standard_2layer`).
//!
//! What they must not do is disagree about the numbers, because then the
//! router lays copper to one fab's rules and the checker measures it against
//! another's. Ten of the checker's thirteen fields exist in the rules crate's
//! constraints; this compares those ten for every fab both tables know.
//!
//! The three the checker has and the constraints do not - `min_via_diameter`,
//! `min_silk_clearance`, `min_courtyard_clearance` - are what stands between
//! this and one table. That is written up in docs/TRACKER.md under V4.

use cypcb_drc::Preset;
use cypcb_rules::presets::RulesPreset;

/// The same fab, named by each table.
const SAME_FAB: &[(Preset, RulesPreset, &str)] = &[
    (
        Preset::Jlcpcb2Layer,
        RulesPreset::JlcpcbStandard2Layer,
        "JLCPCB standard, 2 layers",
    ),
    (
        Preset::Jlcpcb4Layer,
        RulesPreset::JlcpcbStandard4Layer,
        "JLCPCB standard, 4 layers",
    ),
    (
        Preset::JlcpcbAdvanced2Layer,
        RulesPreset::JlcpcbAdvanced2Layer,
        "JLCPCB advanced, 2 layers",
    ),
    (
        Preset::JlcpcbAdvanced4Layer,
        RulesPreset::JlcpcbAdvanced4Layer,
        "JLCPCB advanced, 4 layers",
    ),
    (
        Preset::OshPark2Layer,
        RulesPreset::OshPark2Layer,
        "OSHPark, 2 layers",
    ),
    (
        Preset::OshPark4Layer,
        RulesPreset::OshPark4Layer,
        "OSHPark, 4 layers",
    ),
    (
        Preset::PcbwayStandard,
        RulesPreset::PcbWayStandard,
        "PCBWay standard",
    ),
];

#[test]
fn every_fab_both_tables_know_is_the_same_board() {
    let mut disagreements: Vec<String> = Vec::new();

    for (checker, router, what) in SAME_FAB {
        let rules = checker.rules();
        let constraints = router.constraints();

        let pairs: [(&str, i64, i64); 10] = [
            (
                "min_clearance",
                rules.min_clearance.0,
                constraints.min_clearance.0,
            ),
            (
                "min_trace_width",
                rules.min_trace_width.0,
                constraints.min_trace_width.0,
            ),
            (
                "min_drill_size",
                rules.min_drill_size.0,
                constraints.min_drill_size.0,
            ),
            (
                "min_via_drill",
                rules.min_via_drill.0,
                constraints.min_via_drill.0,
            ),
            (
                "min_annular_ring",
                rules.min_annular_ring.0,
                constraints.min_annular_ring.0,
            ),
            (
                "min_silk_width",
                rules.min_silk_width.0,
                constraints.min_silk_width.0,
            ),
            (
                "min_edge_clearance",
                rules.min_edge_clearance.0,
                constraints.min_edge_clearance.0,
            ),
            (
                "min_hole_to_hole",
                rules.min_hole_to_hole.0,
                constraints.min_hole_to_hole.0,
            ),
            (
                "min_solder_mask_bridge",
                rules.min_solder_mask_bridge.0,
                constraints.min_solder_mask_bridge.0,
            ),
            (
                "solder_mask_expansion",
                rules.solder_mask_expansion.0,
                constraints.solder_mask_expansion.0,
            ),
        ];

        for (field, checker_value, router_value) in pairs {
            if checker_value != router_value {
                disagreements.push(format!(
                    "{what}: {field} is {checker_value}nm to the checker and {router_value}nm to the router"
                ));
            }
        }
    }

    assert!(
        disagreements.is_empty(),
        "the router and the checker disagree about the fab:\n  {}",
        disagreements.join("\n  ")
    );
}

#[test]
fn the_two_tables_still_disagree_about_which_fabs_exist() {
    // Not a wish: a record of the state, so the day somebody merges the two
    // registries this test fails and says what changed. The numbers here are
    // what `Preset::all()` and `RulesPreset::all()` return today.
    assert_eq!(
        Preset::all().len(),
        8,
        "the checker's table changed size; if the registries were merged, delete this test"
    );
    assert_eq!(
        RulesPreset::all().len(),
        10,
        "the router's table changed size; if the registries were merged, delete this test"
    );

    // The three the checker knows about and the router does not, and the other
    // way round, by name.
    assert!(
        Preset::from_name("prototype").is_some() && RulesPreset::from_name("prototype").is_none(),
        "`prototype` is the checker's alone"
    );
    for ipc in ["ipc1", "ipc2", "ipc3"] {
        assert!(
            RulesPreset::from_name(ipc).is_some() && Preset::from_name(ipc).is_none(),
            "{ipc} is the router's alone, and every CLI command refuses it"
        );
    }
}
