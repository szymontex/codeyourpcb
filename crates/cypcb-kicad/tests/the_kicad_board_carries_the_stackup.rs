//! The board a designer opens in KiCad carries the stack they described.
//!
//! `cargo test -p cypcb-kicad --test the_kicad_board_carries_the_stackup`
//!
//! The writer put the stackup's total in `(general (thickness N))` and threw
//! the rest away, so a design that named its layers and its laminate opened in
//! pcbnew as an unnamed default stack. That is the round trip this project was
//! asked for parity on.
//!
//! Every token here was read out of KiCad's own writer rather than guessed.
//! `BOARD_STACKUP::FormatBoardStackup` in `pcbnew/board_stackup_manager/
//! board_stackup.cpp` prints `(layer %s (type %s)` with both quoted, then
//! thickness, then material; `BOARD_STACKUP_ITEM::GetTypeName` is where
//! `copper`, `core`, `prepreg`, `soldermask` and `silkscreen` come from. The
//! comment beside the `(setup ...)` node in `board_writer.rs` records what
//! happens when this is guessed instead: KiCad refuses the whole file, and
//! this project's own importer reading it back happily proves nothing.

use cypcb_core::Nm;
use cypcb_kicad::board_writer::write_board;
use cypcb_world::{BoardWorld, Stackup, StackupLayer, StackupLayerKind};

use StackupLayerKind::{Copper, Core, Mask, Prepreg, Silk};

/// A layer as a design writes one: kind, thickness in mm, optional name and
/// material.
type Spec = (
    StackupLayerKind,
    Option<f64>,
    Option<&'static str>,
    Option<&'static str>,
);

fn board(layers: &[Spec], copper: u8) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "stacked".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        copper,
    );
    world.set_stackup(Stackup {
        layers: layers
            .iter()
            .map(|(kind, thickness, name, material)| StackupLayer {
                kind: *kind,
                name: name.map(str::to_string),
                thickness: thickness.map(Nm::from_mm),
                material: material.map(str::to_string),
            })
            .collect(),
    });
    world
}

/// Every `(layer ...)` line inside the stackup node, trimmed.
fn stackup_lines(text: &str) -> Vec<String> {
    text.lines()
        .skip_while(|line| !line.contains("(stackup"))
        .skip(1)
        .take_while(|line| line.trim() != ")")
        .map(|line| line.trim().to_string())
        .collect()
}

/// A bare four-layer stack: nothing named, every thickness stated.
const BARE: &[Spec] = &[
    (Copper, Some(0.035), None, None),
    (Prepreg, Some(0.2), None, None),
    (Copper, Some(0.0175), None, None),
    (Core, Some(1.095), None, None),
    (Copper, Some(0.0175), None, None),
    (Prepreg, Some(0.2), None, None),
    (Copper, Some(0.035), None, None),
];

#[test]
fn every_layer_gets_the_name_and_type_pcbnew_writes() {
    let mut world = board(BARE, 4);
    let text = write_board(&mut world, "test");
    assert_eq!(
        stackup_lines(&text),
        vec![
            "(layer \"F.Cu\" (type \"copper\") (thickness 0.035))",
            "(layer \"dielectric 1\" (type \"prepreg\") (thickness 0.2))",
            "(layer \"In1.Cu\" (type \"copper\") (thickness 0.0175))",
            "(layer \"dielectric 2\" (type \"core\") (thickness 1.095))",
            "(layer \"In2.Cu\" (type \"copper\") (thickness 0.0175))",
            "(layer \"dielectric 3\" (type \"prepreg\") (thickness 0.2))",
            "(layer \"B.Cu\" (type \"copper\") (thickness 0.035))",
        ],
        "\n{text}"
    );
}

#[test]
fn the_stackup_sits_inside_the_setup_node() {
    // pcbnew reads the stackup out of `(setup ...)`. Written at the top level
    // it is a token in a position KiCad does not expect, which is how the
    // `(setup (rules ...))` attempt made a board nobody could open.
    let mut world = board(BARE, 4);
    let text = write_board(&mut world, "test");
    let setup = text
        .find("  (setup")
        .unwrap_or_else(|| panic!("no setup node:\n{text}"));
    let stackup = text
        .find("    (stackup")
        .unwrap_or_else(|| panic!("no stackup node:\n{text}"));
    assert!(setup < stackup, "the stackup is outside the setup:\n{text}");
    let close = text[stackup..]
        .find("\n  )\n")
        .unwrap_or_else(|| panic!("the setup node never closes:\n{text}"));
    assert!(close > 0, "\n{text}");
}

#[test]
fn a_name_the_design_stated_wins_over_the_derived_one() {
    // The derived names are a fallback for a design that named nothing. A
    // design that did name its layers was talking to a fabricator, and this
    // writer's job is to carry that rather than to correct it.
    let named: &[Spec] = &[
        (Copper, Some(0.035), Some("TOP"), None),
        (Core, Some(1.53), Some("FR4 core"), Some("Isola 370HR")),
        (Copper, Some(0.035), Some("BOTTOM"), None),
    ];
    let mut world = board(named, 2);
    let text = write_board(&mut world, "test");
    assert_eq!(
        stackup_lines(&text),
        vec![
            "(layer \"TOP\" (type \"copper\") (thickness 0.035))",
            "(layer \"FR4 core\" (type \"core\") (thickness 1.53) (material \"Isola 370HR\"))",
            "(layer \"BOTTOM\" (type \"copper\") (thickness 0.035))",
        ],
        "\n{text}"
    );
}

#[test]
fn the_surface_finishes_take_the_side_they_sit_on() {
    let with_finishes: &[Spec] = &[
        (Silk, Some(0.01), None, None),
        (Mask, Some(0.02), None, None),
        (Copper, Some(0.035), None, None),
        (Core, Some(1.5), None, None),
        (Copper, Some(0.035), None, None),
        (Mask, Some(0.02), None, None),
        (Silk, Some(0.01), None, None),
    ];
    let mut world = board(with_finishes, 2);
    let text = write_board(&mut world, "test");
    assert_eq!(
        stackup_lines(&text),
        vec![
            "(layer \"F.SilkS\" (type \"silkscreen\") (thickness 0.01))",
            "(layer \"F.Mask\" (type \"soldermask\") (thickness 0.02))",
            "(layer \"F.Cu\" (type \"copper\") (thickness 0.035))",
            "(layer \"dielectric 1\" (type \"core\") (thickness 1.5))",
            "(layer \"B.Cu\" (type \"copper\") (thickness 0.035))",
            "(layer \"B.Mask\" (type \"soldermask\") (thickness 0.02))",
            "(layer \"B.SilkS\" (type \"silkscreen\") (thickness 0.01))",
        ],
        "\n{text}"
    );
}

#[test]
fn a_layer_that_stated_no_thickness_writes_none() {
    // Same rule the `.cypcb` writer follows: a thickness invented here is a
    // number the fabricator is quoted on that nobody chose.
    let bare: &[Spec] = &[
        (Copper, None, None, None),
        (Core, Some(1.5), None, None),
        (Copper, None, None, None),
    ];
    let mut world = board(bare, 2);
    let text = write_board(&mut world, "test");
    assert_eq!(
        stackup_lines(&text),
        vec![
            "(layer \"F.Cu\" (type \"copper\"))",
            "(layer \"dielectric 1\" (type \"core\") (thickness 1.5))",
            "(layer \"B.Cu\" (type \"copper\"))",
        ],
        "\n{text}"
    );
}

#[test]
fn a_board_that_stated_no_stackup_writes_no_stackup_node() {
    let mut world = BoardWorld::new();
    world.set_board(
        "plain".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        2,
    );
    let text = write_board(&mut world, "test");
    assert!(
        !text.contains("(stackup"),
        "a stackup was invented for a board that stated none:\n{text}"
    );
}
