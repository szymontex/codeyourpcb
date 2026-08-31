//! A pad's own mask opening survives being written down.
//!
//! `cargo test -p cypcb-cli --test a_pads_mask_opening_survives_the_round_trip`
//!
//! KiCad states `(solder_mask_margin 0.1016)` inside a pad, which is 4 mil,
//! and 124 of the 2623 pads in this repository's KiCad files ask for one - a
//! through-hole connector does, so the mask does not creep onto copper a
//! hand-soldered joint has to wet. The importer has read that since the
//! commit before this one and the language could not say it, so the figure
//! survived an import and not a save.
//!
//! Saying nothing is not saying zero. Nothing takes the fabricator's figure;
//! zero opens the mask to the edge of the copper, on every pad, which is a
//! different board.

use cypcb_core::Nm;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn design_with(mask: &str) -> String {
    format!(
        "version 1\n\
         \n\
         board b {{\n\
         \x20   size 10mm x 10mm\n\
         \x20   layers 2\n\
         }}\n\
         \n\
         footprint F {{\n\
         \x20   courtyard 2mm x 2mm\n\
         \x20   pad 1 rect at 0mm, 0mm size 1mm x 1mm{mask}\n\
         }}\n\
         \n\
         component U1 ic \"F\" {{\n\
         \x20   at 5mm, 5mm\n\
         }}\n"
    )
}

/// The margin the design's one pad asks for, as the board model holds it.
fn margin_of(source: &str) -> Option<Nm> {
    let parsed = cypcb_parser::parse(source);
    assert!(
        parsed.errors.is_empty(),
        "the design does not parse: {:?}",
        parsed.errors
    );
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let sync = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(sync.errors.is_empty(), "sync: {:?}", sync.errors);

    let (_, footprint) = library
        .iter()
        .find(|(name, _)| *name == "F")
        .expect("the design states a footprint called F");
    assert_eq!(footprint.pads.len(), 1);
    footprint.pads[0].mask_margin
}

#[test]
fn the_margin_a_design_states_reaches_the_board() {
    assert_eq!(
        margin_of(&design_with(" mask 0.1016mm")),
        Some(Nm::from_mm(0.1016))
    );
    // A pad that asks for nothing takes the fabricator's figure, and the
    // model says so by holding nothing rather than a zero.
    assert_eq!(margin_of(&design_with("")), None);
    // A pad that asks for zero asked for zero.
    assert_eq!(margin_of(&design_with(" mask 0mm")), Some(Nm(0)));
}

#[test]
fn the_writer_says_the_margin_the_reader_read() {
    let source = design_with(" mask 0.1016mm");
    let parsed = cypcb_parser::parse(&source);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);

    let written = cypcb_world::dsl::board_as_dsl(&mut world);
    assert!(
        written.contains("mask 0.101600mm"),
        "the design written out does not say the opening:\n{written}"
    );
    assert_eq!(
        margin_of(&written),
        Some(Nm::from_mm(0.1016)),
        "the design written out reads back as a different pad"
    );
}

#[test]
fn a_pad_that_asks_for_nothing_is_written_as_asking_for_nothing() {
    let source = design_with("");
    let parsed = cypcb_parser::parse(&source);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);

    let written = cypcb_world::dsl::board_as_dsl(&mut world);
    assert!(
        !written.contains("mask "),
        "a pad that asked for nothing was written asking for something:\n{written}"
    );
}

#[test]
fn a_footprint_out_of_kicad_keeps_the_opening_it_asked_for() {
    // `fab-1X04.kicad_mod` is a pin header whose four pads each state 0.1016.
    // The importer reads it, the model carries it, and this is the half that
    // was missing: the design written from that model says it too.
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../viewer/svg-pcb/kicad-components/fab-1X04.kicad_mod");
    let footprint = cypcb_kicad::import_footprint(&file).expect("the fixture reads");

    let mut world = BoardWorld::new();
    world.set_board(
        "header".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    let mut library = FootprintLibrary::new();
    library.register(cypcb_world::footprint::Footprint {
        name: "header".to_string(),
        ..footprint
    });
    world.set_footprints(library.clone());
    world.spawn_component(
        cypcb_world::components::RefDes::new("J1"),
        cypcb_world::components::Value::new(""),
        cypcb_world::components::Position::from_mm(10.0, 10.0),
        cypcb_world::components::Rotation::ZERO,
        cypcb_world::components::FootprintRef::new("header"),
        cypcb_world::components::NetConnections::new(),
    );

    let written = cypcb_world::dsl::board_as_dsl(&mut world);
    assert_eq!(
        written.matches("mask 0.101600mm").count(),
        4,
        "the four pads that asked for 4 mil are not all in the design:\n{written}"
    );
}
