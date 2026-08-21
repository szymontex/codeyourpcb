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
        })
    );
    // And the bottom one looks the other way, at the dielectric above it.
    assert_eq!(
        stackup.environment_of(3),
        Some(CopperEnvironment::Microstrip {
            height: Nm::from_mm(0.2),
            dk_x1000: 4_600,
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
    let Some(CopperEnvironment::Microstrip { height, dk_x1000 }) = stackup.environment_of(0) else {
        panic!("the top layer is a microstrip");
    };
    let z =
        cypcb_calc::microstrip_ohms_x100(Nm::from_mm(0.35), height, Nm::from_mm(0.035), dk_x1000)
            .expect("a 0.35mm trace on 0.2mm of FR4 has an impedance");
    assert!(
        (4_000..=5_500).contains(&z),
        "a 0.35mm outer trace on 0.2mm of 4.6 laminate should land near 50 ohm, got {z}"
    );
}
