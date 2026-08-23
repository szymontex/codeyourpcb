//! Boards whose layers cannot be mistaken for one another.
//!
//! Three index errors have been shipped in this project and every one was
//! found by a mutation or by running the binary, never by the test meant to
//! cover it. All three had the same cause: **a symmetric stack gives
//! neighbouring layers the same answer**, so a rule reading the wrong layer
//! index produces the right number and the test passes.
//!
//! - the outer layers read the dielectric on the wrong side
//! - `BottomCopper` read as copper entry 0
//! - `Layer::Inner(0)` read as copper entry 0 rather than 1
//!
//! A fixture whose layers all differ turns each of those into a failing
//! assertion. This crate exists so the next rule that reads a layer index has
//! one to hand rather than building a symmetric stack of its own, which is
//! what the two test files that already do this both did.
//!
//! It is a dev-dependency and `publish = false`: nothing that ships links it.

use cypcb_core::Nm;
use cypcb_world::components::{Stackup, StackupLayer, StackupLayerKind};

/// A four-layer stack on which every copper layer answers differently.
///
/// The dielectrics are deliberately uniform - 0.1mm of the same laminate
/// between every pair of coppers - so that what separates the four answers is
/// the **foil**, and a test asserting them is asserting the layer it asked
/// about rather than a difference in the material around it:
///
/// | layer | form | foil |
/// |---|---|---|
/// | `F.Cu` | microstrip over 0.1mm | 0.035mm |
/// | `In1.Cu` | stripline across 0.2mm | 0.0175mm |
/// | `In2.Cu` | stripline across 0.2mm | 0.0250mm |
/// | `B.Cu` | microstrip over 0.1mm | 0.0700mm |
///
/// Both inner layers are genuinely centred, so both are answerable - a stack
/// where they are not is a different fixture, because "this layer has no
/// answer" and "this layer has the wrong answer" are different failures.
///
/// The foil is in the denominator of both forms, so a thicker one is a lower
/// impedance: `F.Cu` reads above `B.Cu` and `In1.Cu` above `In2.Cu`. A test
/// that wants a specific number should compute it rather than assume one; what
/// this fixture promises is that **no two of the four are equal**.
pub fn every_copper_layer_answers_differently() -> Stackup {
    let layer = |kind, thickness_mm: f64, dk: Option<u32>| StackupLayer {
        kind,
        name: None,
        thickness: Some(Nm::from_mm(thickness_mm)),
        material: None,
        dk_x1000: dk,
        df_x1000000: None,
    };
    use StackupLayerKind::{Copper, Prepreg};
    Stackup {
        layers: vec![
            layer(Copper, 0.035, None),
            layer(Prepreg, 0.1, Some(4_200)),
            layer(Copper, 0.0175, None),
            layer(Prepreg, 0.1, Some(4_200)),
            layer(Copper, 0.025, None),
            layer(Prepreg, 0.1, Some(4_200)),
            layer(Copper, 0.07, None),
        ],
        ..Stackup::default()
    }
}

/// The ordinary four-layer build: prepreg outside, a thick core in the middle.
///
/// Neither inner layer is centred between two planes of the same dielectric,
/// so neither is a form the closed solutions cover, and a rule asked what this
/// stack delivers there has to say it cannot answer. That is a different
/// failure from answering wrongly, and a fixture that mixes the two proves
/// neither - which is why this is a second stack rather than a flag on the
/// first.
///
/// Written as a source file by
/// [`an_inner_layer_the_forms_cannot_describe_source`].
pub fn an_inner_layer_the_forms_cannot_describe() -> Stackup {
    let layer = |kind, thickness_mm: f64, dk: Option<u32>| StackupLayer {
        kind,
        name: None,
        thickness: Some(Nm::from_mm(thickness_mm)),
        material: None,
        dk_x1000: dk,
        df_x1000000: None,
    };
    use StackupLayerKind::{Copper, Core, Prepreg};
    Stackup {
        layers: vec![
            layer(Copper, 0.035, None),
            layer(Prepreg, 0.2, Some(4_600)),
            layer(Copper, 0.0175, None),
            layer(Core, 1.095, Some(4_500)),
            layer(Copper, 0.0175, None),
            layer(Prepreg, 0.2, Some(4_600)),
            layer(Copper, 0.035, None),
        ],
        ..Stackup::default()
    }
}

/// [`an_inner_layer_the_forms_cannot_describe`], spelled the way a design
/// writes it.
pub fn an_inner_layer_the_forms_cannot_describe_source() -> String {
    cypcb_world::dsl::stackup_as_dsl(&an_inner_layer_the_forms_cannot_describe())
}

/// The same stack as [`every_copper_layer_answers_differently`], spelled the
/// way a design writes it.
///
/// A test that drives `cypcb check` has a source file, not a `Stackup`, so the
/// value above was reachable from `cypcb-drc`'s unit tests and from nowhere
/// else - and the command-line test that needed a four-layer stack built its
/// own, which is the third time a test file has done that. The text comes from
/// the writer the tool itself uses, so this cannot drift from the value it is
/// written from.
///
/// Indented to sit inside a `board` block:
///
/// ```text
/// board b {
///     size 30mm x 20mm
///     layers 4
/// <this>
/// }
/// ```
pub fn every_copper_layer_answers_differently_source() -> String {
    cypcb_world::dsl::stackup_as_dsl(&every_copper_layer_answers_differently())
}

/// The foil thickness of each copper layer of [`every_copper_layer_answers_differently`],
/// top to bottom.
///
/// Four different numbers, which is the whole promise. A caller asserting
/// against a layer index can check it got the one it asked for without
/// computing an impedance at all.
pub const FOILS_MM: [f64; 4] = [0.035, 0.0175, 0.025, 0.07];
