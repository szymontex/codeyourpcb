//! The stack a design states reaches the host that draws it.
//!
//! `cargo test -p cypcb-render --test the_snapshot_carries_the_stack`
//!
//! Seven pieces of stackup vocabulary landed in this project on 2026-08-22 and
//! 2026-08-23 - what the fabricator does to the board, colour, sheets, units,
//! drill pairs, rigid-flex, and the impedance solver - and the language was the
//! only way to see any of it. A stack is the one part of a design that is a
//! table rather than a list of statements, so it is the part a person most
//! wants to look at rather than read, and the snapshot is how anything reaches
//! the thing that draws.

use cypcb_render::PcbEngine;

/// A four-layer stack that states every field the model has.
const BOARD: &str = r#"version 1

board wearable {
    size 60mm x 20mm
    layers 4
    stackup {
        finish "ENIG"
        edges plated
        pads castellated
        connector bevelled
        impedance controlled
        silk "F.SilkS" 0.01mm color "White"
        mask "F.Mask" 0.02mm color "Matte Black"
        copper 1oz
        prepreg 0.0668mm material "FR4" dk 4.5 sheet 0.0668mm material "FR4" dk 4.5
        copper 0.5oz
        core 1.095mm material "Isola 370HR" dk 3.92 df 0.0089
        copper 0.5oz
        prepreg 0.1336mm material "FR4" dk 4.5
        copper 1oz
        drill Top to Bottom
        drill Top to Inner1
    }
}

flex bend { bounds 20mm, 0mm to 40mm, 20mm layer all }
"#;

fn snapshot() -> cypcb_render::BoardSnapshot {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(BOARD);
    assert!(errors.is_empty(), "{errors}");
    engine.build_snapshot()
}

#[test]
fn the_stack_reaches_the_snapshot_at_all() {
    let snapshot = snapshot();
    let stack = snapshot.stackup.expect("the design states a stack");
    assert_eq!(stack.layers.len(), 9, "nine entries, four of them copper");
}

#[test]
fn every_field_the_language_has_survives_the_trip() {
    let stack = snapshot().stackup.expect("a stack");

    assert_eq!(stack.finish, "ENIG");
    assert!(stack.edges_plated);
    assert!(stack.castellated_pads);
    assert_eq!(stack.edge_connector, "bevelled");
    assert!(stack.impedance_controlled);
    assert_eq!(
        stack.drill_pairs,
        vec![
            ["Top".to_string(), "Bottom".to_string()],
            ["Top".to_string(), "Inner1".to_string()],
        ]
    );

    let mask = &stack.layers[1];
    assert_eq!(mask.kind, "mask");
    assert_eq!(mask.name, "F.Mask");
    assert_eq!(mask.color, "Matte Black");

    let copper = &stack.layers[2];
    assert_eq!(copper.kind, "copper");
    assert_eq!(copper.thickness_nm, Some(34_998), "1oz in nanometres");

    let core = &stack.layers[5];
    assert_eq!(core.material, "Isola 370HR");
    assert_eq!(core.dk_x1000, Some(3_920));
    assert_eq!(core.df_x1000000, Some(8_900));
}

#[test]
fn a_slot_of_several_sheets_arrives_as_several_sheets() {
    // A fabricator hits a target thickness with the prepreg they stock, and a
    // panel that showed only the first sheet would show a thinner board than
    // the one being built.
    let stack = snapshot().stackup.expect("a stack");
    let prepreg = &stack.layers[3];
    assert_eq!(prepreg.sheets_nm, vec![66_800, 66_800]);
    assert_eq!(prepreg.thickness_nm, Some(66_800), "its own first sheet");
    assert_eq!(prepreg.slot_thickness_nm, Some(133_600), "both of them");
}

#[test]
fn the_whole_stack_states_its_thickness() {
    let stack = snapshot().stackup.expect("a stack");
    // Every layer states one, so the sum is a measurement rather than a gap.
    // Written out because a total nobody can check by hand is a number nobody
    // believes: silk 10_000 + mask 20_000 + 1oz 34_998 + two prepreg sheets
    // 133_600 + half-ounce 17_499 + core 1_095_000 + half-ounce 17_499 +
    // prepreg 133_600 + 1oz 34_998.
    let by_hand =
        10_000 + 20_000 + 34_998 + 133_600 + 17_499 + 1_095_000 + 17_499 + 133_600 + 34_998;
    assert_eq!(by_hand, 1_497_194);
    assert_eq!(stack.total_thickness_nm, Some(by_hand));
}

#[test]
fn a_board_that_states_no_stack_sends_none() {
    // The common case, and it must not arrive as an empty stack: a design that
    // said nothing about how it is built is different from one that described
    // a board with no layers.
    let mut engine = PcbEngine::new();
    let errors =
        engine.load_source("version 1\n\nboard t {\n    size 30mm x 20mm\n    layers 2\n}\n");
    assert!(errors.is_empty(), "{errors}");
    assert!(engine.build_snapshot().stackup.is_none());
}

#[test]
fn a_flexible_region_does_not_arrive_as_a_keepout() {
    // The snapshot mapped every zone that was not a pour to a keepout, so a
    // bend made the trip out and came back as an area nothing may enter.
    let zones = snapshot().zones;
    let bend = zones
        .iter()
        .find(|zone| zone.name == "bend")
        .expect("the board declares one");
    assert_eq!(bend.kind, "flex");
}
