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
//! thickness, then material.
//!
//! The `(type ...)` words come from `BuildDefaultStackupList` in the same
//! file, which is what sets the name every board actually carries: `KEY_COPPER`,
//! `KEY_CORE` and `KEY_PREPREG` for copper and dielectric, and the human
//! labels `Top Solder Mask`, `Bottom Solder Mask`, `Top Silk Screen` and
//! `Bottom Silk Screen` for the surface finishes. Reading
//! `BOARD_STACKUP_ITEM`'s constructor instead - which sets `soldermask` and
//! `silkscreen` and is overwritten before anything is written - is how this
//! test file once asserted two wrong tokens.
//!
//! The comment beside the `(setup ...)` node in `board_writer.rs` records the
//! cost of getting this wrong: KiCad refuses the whole file, and this
//! project's own importer reading it back happily proves nothing.

use cypcb_core::Nm;
use cypcb_kicad::board_writer::write_board;
use cypcb_world::{BoardWorld, Stackup, StackupLayer, StackupLayerKind};

use StackupLayerKind::{Copper, Core, Mask, Paste, Prepreg, Silk};

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
            "(layer \"F.SilkS\" (type \"Top Silk Screen\") (thickness 0.01))",
            "(layer \"F.Mask\" (type \"Top Solder Mask\") (thickness 0.02))",
            "(layer \"F.Cu\" (type \"copper\") (thickness 0.035))",
            "(layer \"dielectric 1\" (type \"core\") (thickness 1.5))",
            "(layer \"B.Cu\" (type \"copper\") (thickness 0.035))",
            "(layer \"B.Mask\" (type \"Bottom Solder Mask\") (thickness 0.02))",
            "(layer \"B.SilkS\" (type \"Bottom Silk Screen\") (thickness 0.01))",
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

#[test]
fn a_paste_layer_takes_the_name_and_label_kicad_gives_it() {
    // `BuildDefaultStackupList` puts F_Paste between the silkscreen and the
    // mask and types it `Top Solder Paste`, so a board that declares paste has
    // somewhere to arrive. The Gerber job file leaves paste out; this file
    // does not, because this file is what pcbnew reads.
    let with_paste: &[Spec] = &[
        (Silk, Some(0.01), None, None),
        (Paste, Some(0.1), None, None),
        (Mask, Some(0.02), None, None),
        (Copper, Some(0.035), None, None),
        (Core, Some(1.5), None, None),
        (Copper, Some(0.035), None, None),
        (Mask, Some(0.02), None, None),
        (Paste, Some(0.1), None, None),
        (Silk, Some(0.01), None, None),
    ];
    let mut world = board(with_paste, 2);
    let text = write_board(&mut world, "test");
    let lines = stackup_lines(&text);
    assert_eq!(
        lines[1], "(layer \"F.Paste\" (type \"Top Solder Paste\") (thickness 0.1))",
        "\n{text}"
    );
    assert_eq!(
        lines[7], "(layer \"B.Paste\" (type \"Bottom Solder Paste\") (thickness 0.1))",
        "\n{text}"
    );
}

/// Splice a hand-written stackup into a board this writer produced, so the
/// rest of the file is known to parse and only the node under test is new.
fn with_stackup_node(text: &str, body: &str) -> String {
    let anchor = "  (paper \"A4\")\n";
    assert!(text.contains(anchor), "no anchor to splice at:\n{text}");
    text.replace(
        anchor,
        &format!("{anchor}  (setup\n    (stackup\n{body}    )\n  )\n"),
    )
}

#[test]
fn a_stackup_survives_the_trip_out_and_back() {
    // The whole point of the pair. Before the importer read the node, a board
    // this project exported and read back arrived with no stackup at all -
    // the names and the laminate it had just been given were gone.
    let named: &[Spec] = &[
        (Silk, Some(0.01), None, None),
        (Paste, Some(0.1), None, None),
        (Mask, Some(0.02), None, None),
        (Copper, Some(0.035), None, None),
        (Prepreg, Some(0.2), None, Some("FR4")),
        (Copper, Some(0.0175), None, None),
        (Core, Some(1.095), None, Some("Isola 370HR")),
        (Copper, Some(0.0175), None, None),
        (Prepreg, Some(0.2), None, Some("FR4")),
        (Copper, Some(0.035), None, None),
        (Mask, Some(0.02), None, None),
        (Paste, Some(0.1), None, None),
        (Silk, Some(0.01), None, None),
    ];
    let mut world = board(named, 4);
    let text = write_board(&mut world, "test");

    let result = cypcb_kicad::pcb_parser::parse_kicad_pcb_str(&text).expect("the board parses");
    let back = result.world.stackup().expect("a stackup came back");
    assert!(
        result.metadata.stackup_refusals.is_empty(),
        "{:?}",
        result.metadata.stackup_refusals
    );

    let kinds: Vec<&str> = back.layers.iter().map(|l| l.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "silk", "paste", "mask", "copper", "prepreg", "copper", "core", "copper", "prepreg",
            "copper", "mask", "paste", "silk"
        ],
        "\n{text}"
    );
    assert_eq!(
        back.total_thickness(),
        world.stackup().unwrap().total_thickness()
    );
    assert_eq!(back.layers[6].material.as_deref(), Some("Isola 370HR"));
    // The names are the ones the file carries, which is what a design that
    // stated none is told its layers are called.
    assert_eq!(back.layers[0].name.as_deref(), Some("F.SilkS"));
    assert_eq!(back.layers[5].name.as_deref(), Some("In1.Cu"));
    assert_eq!(back.layers[6].name.as_deref(), Some("dielectric 2"));
}

#[test]
fn a_dielectric_written_as_two_atoms_is_read_as_one_name() {
    // The file format's own grammar gives a layer's opening as `"NAME" |
    // dielectric NUMBER`, so a file may carry the pair unquoted. Both spell
    // the same layer.
    let mut world = board(BARE, 4);
    let plain = write_board(&mut world, "test");
    let spliced = with_stackup_node(
        &plain,
        "      (layer \"F.Cu\" (type \"copper\") (thickness 0.035))\n\
         \x20     (layer dielectric 1 (type \"core\") (thickness 1.53) (material \"FR4\"))\n\
         \x20     (layer \"B.Cu\" (type \"copper\") (thickness 0.035))\n",
    );

    let result = cypcb_kicad::pcb_parser::parse_kicad_pcb_str(&spliced).expect("parses");
    let back = result.world.stackup().expect("a stackup came back");
    assert_eq!(back.layers.len(), 3, "\n{spliced}");
    assert_eq!(back.layers[1].name.as_deref(), Some("dielectric 1"));
    assert_eq!(back.layers[1].material.as_deref(), Some("FR4"));
}

#[test]
fn a_layer_kind_with_no_word_here_is_reported_rather_than_skipped_in_silence() {
    // The channel a KiCad release that adds a layer kind arrives through. A
    // stackup two entries short is a different board, so the omission is
    // stated the way a refused zone already is.
    let mut world = board(BARE, 4);
    let plain = write_board(&mut world, "test");
    let spliced = with_stackup_node(
        &plain,
        "      (layer \"F.Cu\" (type \"copper\") (thickness 0.035))\n\
         \x20     (layer \"F.Wonder\" (type \"wonderstuff\") (thickness 0.1))\n\
         \x20     (layer \"B.Cu\" (type \"copper\") (thickness 0.035))\n",
    );

    let result = cypcb_kicad::pcb_parser::parse_kicad_pcb_str(&spliced).expect("parses");
    let back = result.world.stackup().expect("the rest still comes back");
    assert_eq!(back.layers.len(), 2, "\n{spliced}");
    assert_eq!(
        result.metadata.stackup_refusals,
        vec!["`F.Wonder` is a `wonderstuff`, which has no word here".to_string()],
        "the layer went missing without a word about it"
    );
}

#[test]
fn a_board_with_no_stackup_node_arrives_without_one() {
    let mut world = board(BARE, 4);
    let plain = write_board(&mut world, "test");
    let stripped: String = plain
        .lines()
        .filter(|line| {
            !line.contains("(layer \"") && !line.contains("(stackup") && *line != "  (setup"
        })
        .collect::<Vec<_>>()
        .join("\n");
    let result = cypcb_kicad::pcb_parser::parse_kicad_pcb_str(&stripped).expect("parses");
    assert!(result.world.stackup().is_none(), "\n{stripped}");
    assert!(result.metadata.stackup_refusals.is_empty());
}
