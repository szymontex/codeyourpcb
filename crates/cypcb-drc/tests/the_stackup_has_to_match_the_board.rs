//! A design can no longer say two contradictory things about its own layers.
//!
//! `cargo test -p cypcb-drc --test the_stackup_has_to_match_the_board`
//!
//! `stackup { ... }` parsed into `BoardDef::stackup` and was read by nothing -
//! the last construct in the language in that state. A board could declare
//! `layers 4` and then describe two copper layers, and the exporter, which
//! takes its file count from `layers`, would write four Gerbers for a board
//! whose build instructions say two.
//!
//! This is the rule on its own; `a_stackup_that_lies_is_reported.rs` in
//! `cypcb-cli` drives the same fault from a file through parser and sync, so
//! the whole path is held rather than the last third of it.

use cypcb_core::Nm;
use cypcb_drc::{run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::{BoardWorld, Stackup, StackupLayer, StackupLayerKind};

/// `copper 0.035 core 1.5 copper` written the short way.
fn layers(spec: &[(StackupLayerKind, Option<f64>)]) -> Stackup {
    Stackup {
        layers: spec
            .iter()
            .map(|(kind, thickness)| StackupLayer::new(*kind, thickness.map(Nm::from_mm)))
            .collect(),
    }
}

/// What the stackup rule says about a board with this many copper layers.
fn stackup_faults(copper_layers: u8, stackup: Option<Stackup>) -> Vec<String> {
    let mut world = BoardWorld::new();
    world.set_board(
        "t".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        copper_layers,
    );
    if let Some(stackup) = stackup {
        assert!(world.set_stackup(stackup), "the board takes a stackup");
    }

    run_drc(&mut world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| violation.kind == ViolationKind::Stackup)
        .map(|violation| violation.message)
        .collect()
}

use StackupLayerKind::{Copper, Core, Mask, Silk};

#[test]
fn a_stackup_that_matches_the_layer_count_says_nothing() {
    // The control. A rule that fires on a correct board is worse than no rule.
    let two_layer = layers(&[
        (Copper, Some(0.035)),
        (Core, Some(1.5)),
        (Copper, Some(0.035)),
    ]);

    assert_eq!(stackup_faults(2, Some(two_layer)), Vec::<String>::new());
}

#[test]
fn a_board_with_no_stackup_says_nothing() {
    // Most designs state none and take the fab's, which is not a fault - and
    // is why this rule reports nothing on every existing board.
    assert_eq!(stackup_faults(4, None), Vec::<String>::new());
}

#[test]
fn four_layers_and_a_two_layer_stackup_disagree() {
    let two_layer = layers(&[
        (Copper, Some(0.035)),
        (Core, Some(1.5)),
        (Copper, Some(0.035)),
    ]);

    let faults = stackup_faults(4, Some(two_layer));
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("4 copper layers") && faults[0].contains("describes 2"),
        "the message has to carry both numbers: {}",
        faults[0]
    );
}

#[test]
fn the_message_carries_the_thickness_when_the_design_states_one() {
    // 0.035 + 1.5 + 0.035 = 1.57mm, which is what a person compares against
    // what their fab will build - the check this project deliberately does not
    // make on their behalf, because it has no fab thickness table.
    let two_layer = layers(&[
        (Copper, Some(0.035)),
        (Core, Some(1.5)),
        (Copper, Some(0.035)),
    ]);

    let faults = stackup_faults(4, Some(two_layer));
    assert!(faults[0].contains("1.570mm of material"), "{}", faults[0]);
}

#[test]
fn a_stackup_with_no_thicknesses_reports_no_total() {
    // Thickness is optional in the grammar, and a partial sum reads like a
    // measurement rather than like a gap in the design.
    let bare = layers(&[(Copper, None), (Core, None), (Copper, None)]);

    let faults = stackup_faults(4, Some(bare));
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(!faults[0].contains("of material"), "{}", faults[0]);
}

#[test]
fn one_stated_thickness_among_many_is_not_a_total() {
    let partial = layers(&[(Copper, Some(0.035)), (Core, None), (Copper, None)]);

    assert!(
        !stackup_faults(4, Some(partial))[0].contains("of material"),
        "a sum missing two of its three terms is not a thickness"
    );
}

#[test]
fn two_copper_layers_pressed_together_are_reported() {
    // Two foils with nothing between them are one thicker foil: the board's
    // layers are shorted to each other before anybody routes a trace.
    let pressed = layers(&[
        (Copper, Some(0.035)),
        (Copper, Some(0.035)),
        (Core, Some(1.5)),
        (Copper, Some(0.035)),
        (Copper, Some(0.035)),
    ]);

    let faults = stackup_faults(4, Some(pressed));
    assert_eq!(faults.len(), 2, "one per pair: {faults:?}");
    assert!(faults[0].contains("layer 1 and layer 2"), "{}", faults[0]);
    assert!(faults[1].contains("layer 4 and layer 5"), "{}", faults[1]);
}

#[test]
fn a_solder_mask_does_not_separate_two_copper_layers() {
    // The likelier mistake than `copper copper`, and the reason the rule asks
    // what a layer is rather than only whether one is there.
    let masked = layers(&[
        (Copper, Some(0.035)),
        (Mask, Some(0.01)),
        (Copper, Some(0.035)),
    ]);

    let faults = stackup_faults(2, Some(masked));
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("mask is a surface finish"),
        "it has to say which layer was expected to separate them: {}",
        faults[0]
    );
}

#[test]
fn silk_over_copper_in_a_stackup_is_the_same_mistake() {
    let silked = layers(&[
        (Silk, Some(0.01)),
        (Copper, Some(0.035)),
        (Silk, Some(0.01)),
        (Copper, Some(0.035)),
        (Core, Some(1.5)),
    ]);

    let faults = stackup_faults(2, Some(silked));
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("silk is a surface finish"),
        "{}",
        faults[0]
    );
}
