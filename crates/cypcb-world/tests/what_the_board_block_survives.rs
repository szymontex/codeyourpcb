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

/// The same stack, with the two names a fabricator needs on it.
const FOUR_LAYER_NAMED: &str = r#"version 1

board t {
    size 40mm x 20mm
    layers 4
    stackup {
        copper "F.Cu" 0.035mm
        prepreg "dielectric 1" 0.2mm material "FR4"
        copper "In1.Cu" 0.0175mm
        core "dielectric 2" 1.095mm material "Isola 370HR"
        copper "In2.Cu" 0.0175mm
        prepreg "dielectric 3" 0.2mm material "FR4"
        copper "B.Cu" 0.035mm
    }
}
"#;

#[test]
fn a_layer_says_which_layer_it_is_and_what_it_is_made_of() {
    // Before this, a stackup was seven anonymous slabs: a reader could count
    // the copper and add up the thickness and could not say which entry was
    // `In1.Cu`, nor which laminate the board is quoted on. Both names are
    // quoted in the language because a fabricator's canonical layer names
    // carry a dot, which no identifier here may.
    let world = load(FOUR_LAYER_NAMED);
    let stackup = world.stackup().expect("the premise");
    let names: Vec<Option<&str>> = stackup
        .layers
        .iter()
        .map(|layer| layer.name.as_deref())
        .collect();
    assert_eq!(
        names,
        vec![
            Some("F.Cu"),
            Some("dielectric 1"),
            Some("In1.Cu"),
            Some("dielectric 2"),
            Some("In2.Cu"),
            Some("dielectric 3"),
            Some("B.Cu"),
        ]
    );
    assert_eq!(stackup.layers[3].material.as_deref(), Some("Isola 370HR"));
    assert_eq!(stackup.layers[0].material, None, "copper names no laminate");
}

#[test]
fn the_two_names_survive_being_written_down() {
    let mut world = load(FOUR_LAYER_NAMED);
    let before = world.stackup().cloned().expect("the premise");

    let text = board_as_dsl(&mut world);
    assert!(
        text.contains("copper \"In1.Cu\" 0.017500mm"),
        "a layer name has to come back quoted, not dropped:\n{text}"
    );
    assert!(
        text.contains("material \"Isola 370HR\""),
        "a laminate has to come back:\n{text}"
    );

    let back = load(&text);
    assert_eq!(
        back.stackup().cloned(),
        Some(before),
        "the stackup changed on the way out:\n{text}"
    );
}

#[test]
fn a_layer_can_still_say_nothing_but_its_kind() {
    // Both names are optional and the old spelling is still the whole of most
    // designs, so this is the guard against a grammar change that quietly
    // makes them required.
    let world = load(FOUR_LAYER);
    let stackup = world.stackup().expect("the premise");
    assert!(stackup.layers.iter().all(|layer| layer.name.is_none()));
    assert!(stackup.layers.iter().all(|layer| layer.material.is_none()));
}

#[test]
fn solder_paste_is_a_layer_this_language_can_spell() {
    // Paste is not something a fabricator presses, and it is in the language
    // anyway: KiCad's own stackup carries `F.Paste` and `B.Paste` between the
    // silkscreen and the mask, so a board read without a word for one would
    // describe a different build than the file it came from.
    let source = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
    stackup {
        silk 0.01mm
        paste "F.Paste" 0.1mm
        mask 0.02mm
        copper 0.035mm
        core 1.5mm
        copper 0.035mm
        mask 0.02mm
        paste "B.Paste" 0.1mm
        silk 0.01mm
    }
}
"#;
    let mut world = load(source);
    let before = world.stackup().cloned().expect("the premise");
    let kinds: Vec<&str> = before
        .layers
        .iter()
        .map(|layer| layer.kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        vec!["silk", "paste", "mask", "copper", "core", "copper", "mask", "paste", "silk"]
    );

    let text = board_as_dsl(&mut world);
    assert!(text.contains("paste \"F.Paste\" 0.100000mm"), "\n{text}");
    let back = load(&text);
    assert_eq!(back.stackup().cloned(), Some(before), "\n{text}");
}

#[test]
fn the_dielectric_numbers_come_back_as_written() {
    // `dk` and `df` are what a controlled-impedance stack is chosen on, and
    // the two names a laminate datasheet prints them under. They are held in
    // thousandths and millionths rather than as floats, because `StackupLayer`
    // is `Eq` and `Hash` - so this asserts the integers as well as the text,
    // which is where a scale mistake would show.
    let source = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
    stackup {
        copper "F.Cu" 0.035mm
        core "dielectric 1" 1.53mm material "Isola 370HR" dk 3.92 df 0.0089
        copper "B.Cu" 0.035mm
    }
}
"#;
    let mut world = load(source);
    let before = world.stackup().cloned().expect("the premise");
    assert_eq!(before.layers[1].dk_x1000, Some(3_920));
    assert_eq!(before.layers[1].df_x1000000, Some(8_900));

    let text = board_as_dsl(&mut world);
    assert!(
        text.contains("material \"Isola 370HR\" dk 3.92 df 0.0089"),
        "the numbers were rewritten on the way out:\n{text}"
    );

    let back = load(&text);
    assert_eq!(back.stackup().cloned(), Some(before), "\n{text}");
}

#[test]
fn a_dielectric_constant_of_zero_is_refused_rather_than_stored() {
    // A laminate with no permittivity is not a laminate, and a stored nonsense
    // number reads later as a measurement.
    let source = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
    stackup {
        copper 0.035mm
        core 1.53mm dk 0
        copper 0.035mm
    }
}
"#;
    let parsed = cypcb_parser::parse(source);
    assert!(
        !parsed.errors.is_empty(),
        "`dk 0` was accepted: {:?}",
        parsed.errors
    );
}

/// The five things a fabricator does to the board rather than presses into it.
const FAB_ORDER: &str = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
    stackup {
        finish "ENIG"
        edges plated
        pads castellated
        connector bevelled
        impedance controlled
        copper 0.035mm
        core 1.5mm
        copper 0.035mm
    }
}
"#;

#[test]
fn what_the_fabricator_does_to_the_board_comes_back() {
    // KiCad keeps `copper_finish`, `edge_plating`, `castellated_pads`,
    // `edge_connector` and `dielectric_constraints` inside its own stackup,
    // and this project read the file and walked past all five. A board
    // imported and sent back out asked for a different build than it arrived
    // with, and nothing said so.
    let mut world = load(FAB_ORDER);
    let before = world.stackup().cloned().expect("a stackup went in");
    assert_eq!(before.finish.as_deref(), Some("ENIG"), "the premise");
    assert!(before.edges_plated, "the premise");
    assert!(before.castellated_pads, "the premise");
    assert_eq!(
        before.edge_connector,
        Some(cypcb_world::components::EdgeConnector::Bevelled),
        "the premise"
    );
    assert!(before.impedance_controlled, "the premise");

    let text = board_as_dsl(&mut world);
    let back = load(&text);
    assert_eq!(
        back.stackup().cloned(),
        Some(before),
        "the fabrication order did not come back:\n{text}"
    );
}

#[test]
fn a_plain_connector_is_not_a_bevelled_one() {
    // Two words the same rule takes, and the difference is a chamfer a
    // fabricator either cuts or does not.
    let source = FAB_ORDER.replace("connector bevelled", "connector plain");
    let mut world = load(&source);
    assert_eq!(
        world.stackup().and_then(|s| s.edge_connector),
        Some(cypcb_world::components::EdgeConnector::Plain)
    );

    let text = board_as_dsl(&mut world);
    assert!(
        text.contains("connector plain"),
        "the writer states which kind:\n{text}"
    );
}

#[test]
fn a_board_that_asks_for_none_of_it_writes_none_of_it() {
    // Silence is the rest, the way `locked` on a trace works: a design that
    // wants no edge plating does not say `edges plated`, and the writer must
    // not invent a line saying it either way.
    let mut world = load(FOUR_LAYER);
    let stackup = world.stackup().cloned().expect("a stackup went in");
    assert!(!stackup.edges_plated && !stackup.castellated_pads);

    let text = board_as_dsl(&mut world);
    for word in [
        "finish",
        "edges",
        "pads castellated",
        "connector",
        "impedance",
    ] {
        assert!(
            !text.contains(word),
            "a board that stated nothing had `{word}` written for it:\n{text}"
        );
    }
}

/// A stack written the way a fab table is: copper in ounces, dielectrics in
/// microns and mils, the board in inches.
const MIXED_UNITS: &str = r#"version 1

board t {
    size 1.5in x 30mm
    layers 4
    stackup {
        copper 1oz
        prepreg 100um dk 4.2
        copper 0.5oz
        core 43.1mil dk 4.5
        copper 0.5oz
        prepreg 100um dk 4.2
        copper 2oz
    }
}
"#;

#[test]
fn copper_can_be_stated_in_the_unit_it_is_bought_in() {
    // Copper foil is sold by weight per square foot and every fab table in
    // this project states it that way. The language took millimetres and
    // nothing else, so a designer reading `1oz` off a table did the conversion
    // in their head before they could write it down.
    let world = load(MIXED_UNITS);
    let stackup = world.stackup().expect("a stackup went in");
    let coppers: Vec<i64> = stackup
        .layers
        .iter()
        .filter(|layer| layer.kind == cypcb_world::StackupLayerKind::Copper)
        .map(|layer| layer.thickness.expect("stated").raw())
        .collect();

    // 1oz is 34_998nm, which is `cypcb_core::NM_PER_OZ` and what the IPC-2221
    // width calculation reads as well.
    assert_eq!(coppers, vec![34_998, 17_499, 17_499, 69_996], "{coppers:?}");
}

#[test]
fn the_other_two_units_land_where_they_should() {
    let world = load(MIXED_UNITS);
    let stackup = world.stackup().expect("a stackup went in");
    let prepreg = stackup.layers[1].thickness.expect("stated");
    let core = stackup.layers[3].thickness.expect("stated");

    assert_eq!(prepreg.raw(), 100_000, "100um is 0.1mm");
    assert_eq!(core.raw(), 1_094_740, "43.1mil at 25400nm each");
}

#[test]
fn a_thickness_comes_back_in_the_unit_it_was_written_in() {
    // The number is nanometres either way. What this holds is the wording: a
    // fabricator asked for `1oz` should read `1oz` back, not the arithmetic.
    let mut world = load(MIXED_UNITS);
    let text = board_as_dsl(&mut world);

    assert!(text.contains("copper 1oz"), "{text}");
    assert!(text.contains("copper 0.5oz"), "{text}");
    assert!(text.contains("copper 2oz"), "{text}");
    assert!(text.contains("prepreg 100um"), "{text}");
    assert!(text.contains("core 43.1mil"), "{text}");

    // And it still reads back as the same board.
    let back = load(&text);
    assert_eq!(back.stackup(), world.stackup(), "\n{text}");
}

#[test]
fn a_thickness_written_in_millimetres_is_still_written_in_millimetres() {
    // The control. Most stackups state millimetres, and this must not start
    // printing them as something else.
    let mut world = load(FOUR_LAYER);
    let text = board_as_dsl(&mut world);
    // Six decimals, which is 1nm resolution: the writer's rule for
    // millimetres, and the reason a round trip through this file is exact.
    assert!(text.contains("copper 0.035000mm"), "{text}");
    assert!(!text.contains("oz"), "{text}");
}

#[test]
fn ounces_are_a_copper_weight_and_the_reader_says_so() {
    // A weight per square foot is a thickness of copper and of nothing else.
    // Without a message here the loop reads the leftover `oz` as a property
    // name and answers "`stackup` has no property `oz`" - true, and not what
    // happened.
    let source = r#"version 1

board t {
    size 30mm x 20mm
    layers 2
    stackup {
        copper 1oz
        core 1oz
        copper 1oz
    }
}
"#;
    let parsed = cypcb_parser::parse(source);
    let said = format!("{:?}", parsed.errors);
    assert!(
        !parsed.errors.is_empty(),
        "a core in ounces is not a length"
    );
    assert!(
        said.contains("weight of copper"),
        "the message says what ounces are: {said}"
    );
}

/// A board that says what colour it wants to be.
const COLOURED: &str = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
    stackup {
        silk "F.SilkS" 0.01mm color "White"
        mask "F.Mask" 0.02mm color "Matte Black"
        copper 1oz
        core 1.5mm
        copper 1oz
        mask "B.Mask" 0.02mm color "Matte Black"
        silk "B.SilkS" 0.01mm color "White"
    }
}
"#;

#[test]
fn a_mask_can_say_what_colour_it_is() {
    // A solder mask is green unless somebody says otherwise, and a house
    // charges for saying otherwise - so the colour is part of the order.
    // KiCad carries it per stackup layer and this project had no word for it.
    let mut world = load(COLOURED);
    let stackup = world.stackup().cloned().expect("a stackup went in");
    let colours: Vec<Option<&str>> = stackup
        .layers
        .iter()
        .map(|layer| layer.color.as_deref())
        .collect();
    assert_eq!(
        colours,
        vec![
            Some("White"),
            Some("Matte Black"),
            None,
            None,
            None,
            Some("Matte Black"),
            Some("White"),
        ],
        "{colours:?}"
    );

    let text = board_as_dsl(&mut world);
    assert!(text.contains("color \"Matte Black\""), "{text}");
    let back = load(&text);
    assert_eq!(back.stackup().cloned(), Some(stackup), "\n{text}");
}

#[test]
fn a_layer_that_named_no_colour_is_not_given_one() {
    // The control, and the same rule the rest of this writer follows: a colour
    // invented here is a line on an order nobody chose.
    let mut world = load(FOUR_LAYER);
    let text = board_as_dsl(&mut world);
    assert!(!text.contains("color"), "{text}");
}

/// A four-layer stack whose dielectric slots are pressed from several sheets,
/// which is what a fabricator actually does above two layers.
const SHEETED: &str = r#"version 1

board t {
    size 40mm x 20mm
    layers 4
    stackup {
        copper 1oz
        prepreg 0.0668mm material "FR4" dk 4.5 sheet 0.0668mm material "FR4" dk 4.5
        copper 0.5oz
        core 1.095mm material "FR4" dk 4.5
        copper 0.5oz
        prepreg 0.0668mm material "FR4" dk 4.5 sheet 0.0668mm material "FR4" dk 4.5
        copper 1oz
    }
}
"#;

#[test]
fn a_dielectric_slot_can_be_pressed_from_several_sheets() {
    // One slot in a stackup is not one sheet of laminate. A fabricator hits a
    // target thickness by stacking the prepreg they stock, and above two
    // layers that is the ordinary case rather than the exotic one. KiCad
    // writes each extra sheet as `addsublayer` and this project dropped them,
    // so a six-layer board came back thinner than it went out.
    let world = load(SHEETED);
    let stackup = world.stackup().expect("a stackup went in");
    assert_eq!(
        stackup.layers[1].sheets.len(),
        1,
        "one sheet after the first"
    );

    // The slot is both sheets: 0.0668 twice is 0.1336mm.
    assert_eq!(
        stackup.layers[1].slot_thickness().expect("stated").raw(),
        133_600
    );
    // And the layer's own `thickness` is still only its first sheet, so
    // nothing that reads it has silently changed meaning.
    assert_eq!(stackup.layers[1].thickness.expect("stated").raw(), 66_800);
}

#[test]
fn a_slot_of_two_laminates_has_no_dielectric_constant() {
    // The closed-form impedance solutions take one `dk`. A slot pressed from
    // two different laminates has none, so the answer is "not checked" rather
    // than a number picked off whichever sheet came first.
    // Only the first slot: the two prepreg lines are identical, and replacing
    // both would leave nothing uniform to compare against.
    let source = SHEETED.replacen(
        "prepreg 0.0668mm material \"FR4\" dk 4.5 sheet 0.0668mm material \"FR4\" dk 4.5",
        "prepreg 0.0668mm material \"FR4\" dk 4.5 sheet 0.0668mm material \"Isola\" dk 3.92",
        1,
    );
    let world = load(&source);
    let stackup = world.stackup().expect("a stackup went in");
    assert_eq!(stackup.layers[1].slot_dk_x1000(), None);
    // The uniform slot below it still answers.
    assert_eq!(stackup.layers[5].slot_dk_x1000(), Some(4_500));
}

#[test]
fn the_total_thickness_counts_every_sheet() {
    // `Stackup::total_thickness` is the depth every plated hole is drilled
    // through and the figure a fab quotes against, so a slot counted as one
    // sheet understates the board.
    let world = load(SHEETED);
    let stackup = world.stackup().expect("a stackup went in");
    // Copper: 1 + 0.5 + 0.5 + 1 oz. Dielectric: 0.1336 x 2 + 1.095.
    let expected = 34_998 + 17_499 + 17_499 + 34_998 + 133_600 + 133_600 + 1_095_000;
    assert_eq!(stackup.total_thickness().expect("stated").raw(), expected);
}

#[test]
fn the_sheets_survive_being_written_down() {
    let mut world = load(SHEETED);
    let before = world.stackup().cloned().expect("a stackup went in");
    let text = board_as_dsl(&mut world);
    assert!(text.contains("sheet 0.066800mm"), "{text}");
    let back = load(&text);
    assert_eq!(back.stackup().cloned(), Some(before), "\n{text}");
}

#[test]
fn the_drill_pairs_survive_being_written_down() {
    // A board is drilled and plated once per lamination cycle, and each cycle
    // reaches only the layers pressed together by then. Altium calls these
    // drill pairs; KiCad has no word for them, so a design that states them
    // and saves loses the whole build plan unless this writer carries it.
    let source = r#"version 1

board t {
    size 40mm x 20mm
    layers 4
    stackup {
        copper 1oz
        prepreg 0.1mm
        copper 0.5oz
        core 1.095mm
        copper 0.5oz
        prepreg 0.1mm
        copper 1oz
        drill Top to Bottom
        drill Top to Inner1
    }
}
"#;
    let mut world = load(source);
    let before = world.stackup().cloned().expect("a stackup went in");
    assert_eq!(before.drill_pairs.len(), 2, "the premise");

    let text = board_as_dsl(&mut world);
    assert!(text.contains("drill Top to Bottom"), "{text}");
    assert!(text.contains("drill Top to Inner1"), "{text}");

    let back = load(&text);
    assert_eq!(back.stackup().cloned(), Some(before), "\n{text}");
}

#[test]
fn a_drill_pair_naming_a_layer_the_language_does_not_have_is_reported() {
    // The same rule a trace's layer follows: a name nobody can read is a hole
    // nobody checks, so it is reported rather than dropped.
    let source = r#"version 1

board t {
    size 40mm x 20mm
    layers 4
    stackup {
        copper 1oz
        core 1.5mm
        copper 1oz
        drill Top to Inner9
    }
}
"#;
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    let said = format!("{:?}", result.errors);
    assert!(!result.errors.is_empty(), "Inner9 is not a layer");
    assert!(said.contains("Top to Inner9"), "{said}");
}

#[test]
fn a_flex_stack_can_name_its_coverlay_and_its_stiffener() {
    // What a solder mask is on a rigid board, coverlay is on a flexible one -
    // and not the same thing: mask is a liquid cured in place and cracks when
    // the board bends, so a flexible section gets a film laminated over it. A
    // stiffener is bonded under the part of the flex that must not bend.
    let source = r#"version 1

board wearable {
    size 60mm x 20mm
    layers 2
    stackup {
        coverlay 0.025mm material "Kapton"
        copper 0.5oz
        core 0.05mm material "Kapton" dk 3.4
        copper 0.5oz
        coverlay 0.025mm material "Kapton"
        stiffener 0.2mm material "FR4"
    }
}
"#;
    let mut world = load(source);
    let before = world.stackup().cloned().expect("a stackup went in");
    let kinds: Vec<String> = before
        .layers
        .iter()
        .map(|layer| layer.kind.as_str().to_string())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "coverlay",
            "copper",
            "core",
            "copper",
            "coverlay",
            "stiffener"
        ],
        "{kinds:?}"
    );

    let text = board_as_dsl(&mut world);
    assert!(text.contains("coverlay 0.025000mm"), "{text}");
    assert!(text.contains("stiffener 0.200000mm"), "{text}");

    let back = load(&text);
    assert_eq!(back.stackup().cloned(), Some(before), "\n{text}");
}
