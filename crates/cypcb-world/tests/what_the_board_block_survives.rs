//! Which part of the board block does not survive being written down.
//!
//! `cargo test -p cypcb-world --test what_the_board_block_survives`
//!
//! The trace round trip has its own file. This one asks the same question of
//! the board block itself, and the answer when it was written was that the
//! stackup did not come back: seven layers went in and `None` came out. A
//! design that states how it wants to be built lost that statement on its
//! first save through the editor, silently - the shape of defect this project
//! has already been bitten by on traces and on net names.

use cypcb_core::Nm;
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn load(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);
    world
}

/// A four-layer stack with every thickness stated - what a design sends to a
/// fabricator when it cares how the board is built.
const FOUR_LAYER: &str = r#"version 1

board t {
    size 40mm x 20mm
    layers 4
    stackup {
        copper 0.035mm
        prepreg 0.2mm
        copper 0.0175mm
        core 1.095mm
        copper 0.0175mm
        prepreg 0.2mm
        copper 0.035mm
    }
}
"#;

#[test]
fn a_stackup_comes_back_layer_for_layer() {
    let mut world = load(FOUR_LAYER);
    let before = world.stackup().cloned();
    let before = before.expect("the premise: a stackup went in");
    assert_eq!(before.layers.len(), 7, "the premise: seven layers");
    assert_eq!(before.copper_count(), 4, "the premise: four of them copper");

    let text = board_as_dsl(&mut world);
    let back = load(&text);
    assert_eq!(
        back.stackup().cloned(),
        Some(before),
        "the stackup did not come back:\n{text}"
    );
}

#[test]
fn the_thickness_a_fab_is_quoted_on_survives() {
    // `total_thickness` is the depth every plated hole is drilled through, and
    // it is `None` the moment one layer leaves its thickness unsaid - so this
    // asserts the number rather than the layer list, which is the form the
    // KiCad writer actually consumes.
    let mut world = load(FOUR_LAYER);
    let before = world
        .stackup()
        .and_then(|stackup| stackup.total_thickness())
        .expect("the premise: every layer stated a thickness");
    assert_eq!(before, Nm::from_mm(1.6), "the premise: a 1.6mm board");

    let text = board_as_dsl(&mut world);
    let back = load(&text);
    assert_eq!(
        back.stackup().and_then(|stackup| stackup.total_thickness()),
        Some(before),
        "the board's own thickness changed on the way out:\n{text}"
    );
}

#[test]
fn a_layer_that_stated_no_thickness_is_not_given_one() {
    // The tempting bug: write a plausible foil thickness for a layer that left
    // it unsaid, so the file looks complete. That turns a gap in the design
    // into a number the fabricator is quoted on, and `total_thickness` stops
    // being able to say "this design did not state one".
    let source = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
    stackup {
        copper
        core 1.5mm
        copper 0.035mm
    }
}
"#;
    let mut world = load(source);
    assert_eq!(
        world
            .stackup()
            .and_then(|stackup| stackup.total_thickness()),
        None,
        "the premise: one layer left its thickness unsaid"
    );

    let text = board_as_dsl(&mut world);
    assert!(
        text.contains("        copper\n"),
        "the bare layer was given a thickness it never stated:\n{text}"
    );
    let back = load(&text);
    assert_eq!(
        back.stackup().and_then(|stackup| stackup.total_thickness()),
        None,
        "a design that stated no thickness now reports one:\n{text}"
    );
    assert_eq!(
        back.stackup().map(|stackup| stackup.layers.len()),
        Some(3),
        "\n{text}"
    );
}

#[test]
fn a_board_that_stated_no_stackup_is_not_given_one() {
    // The other direction, and the same rule the `fab` line already follows:
    // this writer returns what it was given. Inventing a stackup here would
    // make every round trip claim a choice the source never made - and the
    // checker grades a stated stackup against the layer count, so an invented
    // one is an invented verdict.
    let source = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
}
"#;
    let mut world = load(source);
    assert!(world.stackup().is_none(), "the premise");

    let text = board_as_dsl(&mut world);
    assert!(
        !text.contains("stackup"),
        "a stackup was invented for a board that stated none:\n{text}"
    );
    let back = load(&text);
    assert!(back.stackup().is_none(), "\n{text}");
}
