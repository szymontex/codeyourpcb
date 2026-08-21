//! Which impedance form each copper layer of a stack calls for.
//!
//! `cargo test -p cypcb-world --test where_a_copper_layer_sits`
//!
//! The impedance forms in `cypcb-calc` need geometry the stackup already
//! holds: how far a trace is from its reference plane and what is in between.
//! This is the lookup, and the interesting half of it is what it refuses.
//!
//! A symmetric stripline is symmetric. A trace nearer one plane than the other
//! is an asymmetric stripline and a different equation, which this project
//! does not have - so an inner layer with prepreg on one side and core on the
//! other answers nothing rather than answering with the wrong form. That is
//! most four-layer stacks, and it is the correct answer for them.

use cypcb_core::Nm;
use cypcb_world::components::{CopperEnvironment, Stackup, StackupLayer, StackupLayerKind};

use StackupLayerKind::{Copper, Core, Mask, Prepreg, Silk};

/// kind, thickness in mm, dk in thousandths.
type Spec = (StackupLayerKind, Option<f64>, Option<u32>);

fn stack(layers: &[Spec]) -> Stackup {
    Stackup {
        layers: layers
            .iter()
            .map(|(kind, thickness, dk)| StackupLayer {
                kind: *kind,
                name: None,
                thickness: thickness.map(Nm::from_mm),
                material: None,
                dk_x1000: *dk,
                df_x1000000: None,
            })
            .collect(),
    }
}

/// A four-layer stack whose inner layers are genuinely centred: the same
/// dielectric above and below each of them.
const CENTRED: &[Spec] = &[
    (Silk, Some(0.01), None),
    (Mask, Some(0.02), None),
    (Copper, Some(0.035), None),
    (Prepreg, Some(0.2), Some(4_600)),
    (Copper, Some(0.0175), None),
    (Prepreg, Some(0.2), Some(4_600)),
    (Copper, Some(0.0175), None),
    (Prepreg, Some(0.2), Some(4_600)),
    (Copper, Some(0.035), None),
    (Mask, Some(0.02), None),
    (Silk, Some(0.01), None),
];

#[test]
fn an_outer_layer_is_a_microstrip_over_the_dielectric_under_it() {
    let stackup = stack(CENTRED);
    assert_eq!(
        stackup.environment_of(0),
        Some(CopperEnvironment::Microstrip {
            height: Nm::from_mm(0.2),
            dk_x1000: 4_600,
            copper: Nm::from_mm(0.035),
        })
    );
    // And the bottom one looks the other way, at the dielectric above it.
    assert_eq!(
        stackup.environment_of(3),
        Some(CopperEnvironment::Microstrip {
            height: Nm::from_mm(0.2),
            dk_x1000: 4_600,
            copper: Nm::from_mm(0.035),
        })
    );
}

#[test]
fn a_centred_inner_layer_is_a_stripline_between_both_planes() {
    // The separation is both dielectrics, not one: the form's `B` is the
    // distance between the planes, and the trace sits halfway.
    let stackup = stack(CENTRED);
    assert_eq!(
        stackup.environment_of(1),
        Some(CopperEnvironment::Stripline {
            plate_separation: Nm::from_mm(0.4),
            dk_x1000: 4_600,
            copper: Nm::from_mm(0.0175),
        })
    );
    assert_eq!(stackup.environment_of(2), stackup.environment_of(1));
}

#[test]
fn an_inner_layer_that_is_not_centred_answers_nothing() {
    // The ordinary four-layer build: prepreg against the outer layers and a
    // thick core in the middle. L2 has 0.2mm of prepreg above it and 1.095mm
    // of core below, so it is not halfway between its planes and the
    // symmetric form does not describe it.
    let ordinary: &[Spec] = &[
        (Copper, Some(0.035), None),
        (Prepreg, Some(0.2), Some(4_600)),
        (Copper, Some(0.0175), None),
        (Core, Some(1.095), Some(4_500)),
        (Copper, Some(0.0175), None),
        (Prepreg, Some(0.2), Some(4_600)),
        (Copper, Some(0.035), None),
    ];
    let stackup = stack(ordinary);
    assert_eq!(stackup.environment_of(1), None, "L2 is not centred");
    assert_eq!(stackup.environment_of(2), None, "nor is L3");
    // The outer layers are unaffected: each still has one dielectric inward.
    assert!(stackup.environment_of(0).is_some());
    assert!(stackup.environment_of(3).is_some());
}

#[test]
fn a_centred_layer_between_two_different_laminates_answers_nothing() {
    // Same thickness either side, different dielectric constant. The form
    // assumes one dielectric; two is a case it does not cover, and averaging
    // them would be this tool inventing a laminate.
    let mixed: &[Spec] = &[
        (Copper, Some(0.035), None),
        (Prepreg, Some(0.2), Some(4_600)),
        (Copper, Some(0.0175), None),
        (Core, Some(0.2), Some(3_500)),
        (Copper, Some(0.035), None),
    ];
    assert_eq!(stack(mixed).environment_of(1), None);
}

#[test]
fn a_dielectric_that_states_no_dk_answers_nothing() {
    // A stack can be complete about its thicknesses and silent about its
    // laminate, and most are. Silence is not 4.5.
    let silent: &[Spec] = &[
        (Copper, Some(0.035), None),
        (Core, Some(1.5), None),
        (Copper, Some(0.035), None),
    ];
    assert_eq!(stack(silent).environment_of(0), None);

    let unmeasured: &[Spec] = &[
        (Copper, Some(0.035), None),
        (Core, None, Some(4_500)),
        (Copper, Some(0.035), None),
    ];
    assert_eq!(stack(unmeasured).environment_of(0), None);
}

#[test]
fn a_layer_that_is_not_there_answers_nothing() {
    let stackup = stack(CENTRED);
    assert_eq!(
        stackup.environment_of(4),
        None,
        "a four-copper stack has 0..3"
    );
}

#[test]
fn the_two_forms_agree_with_the_calculator_they_were_built_for() {
    // The point of the lookup: what comes out goes straight into
    // `cypcb-calc`. This is the join, and it is the only test here that knows
    // both sides.
    let stackup = stack(CENTRED);
    let Some(CopperEnvironment::Microstrip {
        height,
        dk_x1000,
        copper,
    }) = stackup.environment_of(0)
    else {
        panic!("the top layer is a microstrip");
    };
    let z = cypcb_calc::microstrip_ohms_x100(Nm::from_mm(0.35), height, copper, dk_x1000)
        .expect("a 0.35mm trace on 0.2mm of FR4 has an impedance");
    assert!(
        (4_000..=5_500).contains(&z),
        "a 0.35mm outer trace on 0.2mm of 4.6 laminate should land near 50 ohm, got {z}"
    );
}

#[test]
fn the_shared_fixture_gives_each_copper_layer_its_own_answer() {
    // The guard on `cypcb-fixtures` itself. Its whole promise is that no two
    // copper layers of that stack look alike, so a rule reading the wrong
    // layer index cannot produce the right number by accident. If the fixture
    // ever loses that property it stops being able to catch the thing it was
    // built for, silently, and every test resting on it goes quiet with it.
    let stackup = cypcb_fixtures::every_copper_layer_answers_differently();

    let foils: Vec<Option<Nm>> = (0..4)
        .map(|index| match stackup.environment_of(index) {
            Some(CopperEnvironment::Microstrip { copper, .. }) => Some(copper),
            Some(CopperEnvironment::Stripline { copper, .. }) => Some(copper),
            None => None,
        })
        .collect();
    let expected: Vec<Option<Nm>> = cypcb_fixtures::FOILS_MM
        .iter()
        .map(|mm| Some(Nm::from_mm(*mm)))
        .collect();
    assert_eq!(foils, expected, "every layer answers with its own foil");

    // And all four are answerable: "no answer" and "the wrong answer" are
    // different failures, and this fixture is for finding the second.
    for index in 0..4 {
        assert!(
            stackup.environment_of(index).is_some(),
            "copper entry {index} has no answer"
        );
    }
    // The two inner layers are striplines and the two outer ones are not.
    assert!(matches!(
        stackup.environment_of(0),
        Some(CopperEnvironment::Microstrip { .. })
    ));
    assert!(matches!(
        stackup.environment_of(1),
        Some(CopperEnvironment::Stripline { .. })
    ));
    assert!(matches!(
        stackup.environment_of(2),
        Some(CopperEnvironment::Stripline { .. })
    ));
    assert!(matches!(
        stackup.environment_of(3),
        Some(CopperEnvironment::Microstrip { .. })
    ));
}
