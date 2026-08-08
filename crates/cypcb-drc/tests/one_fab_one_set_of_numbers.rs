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

use cypcb_core::Nm;
use cypcb_drc::{Preset, PresetRules};
use cypcb_rules::presets::RulesPreset;

/// The same fab, named by each table.
const SAME_FAB: &[(Preset, RulesPreset, &str)] = &[
    (
        Preset::JlcpcbStandard2Layer,
        RulesPreset::JlcpcbStandard2Layer,
        "JLCPCB standard, 2 layers",
    ),
    (
        Preset::JlcpcbStandard4Layer,
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
        Preset::PcbWayStandard,
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
fn there_is_one_list_of_presets() {
    // This file used to record a split: two enums, eight fabs against ten,
    // `prototype` known only to the checker and `ipc1`/`ipc2`/`ipc3` only to
    // the router, which every CLI command refused. They are one list now, and
    // this is what keeps them one - `cypcb_drc::Preset` is the same type, so
    // the comparison above compares a preset with itself and every name either
    // table ever accepted still resolves.
    assert_eq!(
        std::any::TypeId::of::<Preset>(),
        std::any::TypeId::of::<RulesPreset>(),
        "the checker grew its own preset enum again"
    );

    for name in [
        "jlcpcb",
        "jlcpcb_2layer",
        "jlcpcb_standard_2layer",
        "jlcpcb_4layer",
        "jlcpcb_advanced",
        "oshpark",
        "oshpark_4layer",
        "pcbway",
        "prototype",
        "ipc1",
        "ipc2",
        "ipc3",
    ] {
        assert!(
            Preset::from_name(name).is_some(),
            "`--preset {name}` used to work, or was documented and refused"
        );
    }
}

#[test]
fn prototype_kept_every_number_it_had() {
    // `prototype` is not a fab: it was thirteen hand-written numbers in the
    // checker's own table, and the merge moved it into the shared one. Moving
    // a preset must not change what it checks, so this is the whole table.
    let rules = Preset::Prototype.rules();

    assert_eq!(rules.min_clearance, Nm::from_mm(0.2));
    assert_eq!(rules.min_trace_width, Nm::from_mm(0.25));
    assert_eq!(rules.min_drill_size, Nm::from_mm(0.4));
    assert_eq!(rules.min_via_drill, Nm::from_mm(0.3));
    assert_eq!(rules.min_via_diameter, Nm::from_mm(0.8));
    assert_eq!(rules.min_annular_ring, Nm::from_mm(0.2));
    assert_eq!(rules.min_silk_width, Nm::from_mm(0.2));
    assert_eq!(rules.min_edge_clearance, Nm::from_mm(0.5));
    assert_eq!(rules.min_hole_to_hole, Nm::from_mm(0.6));
    assert_eq!(rules.min_solder_mask_bridge, Nm::from_mm(0.15));
    assert_eq!(rules.solder_mask_expansion, Nm::from_mm(0.075));
    assert_eq!(rules.min_silk_clearance, Nm::from_mm(0.2));
    assert_eq!(rules.min_courtyard_clearance, Nm::from_mm(0.5));
}
