//! A pad turned inside its footprint survives being written down.
//!
//! `cargo test -p cypcb-cli --test a_turned_pad_survives_the_dsl_round_trip`
//!
//! The board model has held a pad's own turn since the KiCad importer learned
//! to read one: a 1x4 header whose oblong pads stand across the part, a module
//! whose edge pads face outwards. The language could not say it, so a board
//! imported from KiCad and saved here came back with every such pad standing
//! the other way, and a footprint written by hand had to swap the pad's width
//! and height instead - which is a different pad the moment it has a slot.

use cypcb_core::{Nm, Point};
use cypcb_world::components::Rotation;
use cypcb_world::footprint::{Footprint, FootprintLibrary};
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn design_with(turn: &str) -> String {
    format!(
        "version 1\n\
         \n\
         board b {{\n\
         \x20   size 20mm x 20mm\n\
         \x20   layers 2\n\
         }}\n\
         \n\
         footprint F {{\n\
         \x20   courtyard 6mm x 6mm\n\
         \x20   pad 1 oblong at 0mm, 0mm{turn} size 1mm x 3mm drill 0.8mm x 2mm\n\
         }}\n\
         \n\
         component U1 ic \"F\" {{\n\
         \x20   at 10mm, 10mm\n\
         }}\n"
    )
}

fn footprint_of(source: &str) -> (BoardWorld, Footprint) {
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
    let footprint = footprint.clone();
    (world, footprint)
}

#[test]
fn the_turn_a_design_states_reaches_the_board() {
    let (_, turned) = footprint_of(&design_with(" rotate 90"));
    assert_eq!(turned.pads[0].rotation, Rotation::from_degrees(90.0));
    // The size stays the pad's own; the turn is carried beside it.
    assert_eq!(turned.pads[0].size, (Nm::from_mm(1.0), Nm::from_mm(3.0)));

    let (_, square) = footprint_of(&design_with(""));
    assert_eq!(square.pads[0].rotation, Rotation::ZERO);
}

#[test]
fn the_footprint_spans_its_pad_as_it_stands() {
    // A 1mm x 3mm pad turned a quarter is 3mm across and 1mm tall, and the
    // footprint's extent is measured over the pad on the part, not over the
    // numbers it was written with.
    let (_, turned) = footprint_of(&design_with(" rotate 90"));
    assert_eq!(turned.bounds.width(), Nm::from_mm(3.0));
    assert_eq!(turned.bounds.height(), Nm::from_mm(1.0));
    assert_eq!(turned.bounds.center(), Point::ORIGIN);

    let (_, square) = footprint_of(&design_with(""));
    assert_eq!(square.bounds.width(), Nm::from_mm(1.0));
    assert_eq!(square.bounds.height(), Nm::from_mm(3.0));
}

#[test]
fn the_writer_says_the_turn_the_reader_read() {
    let (mut world, _) = footprint_of(&design_with(" rotate 90"));
    let written = cypcb_world::dsl::board_as_dsl(&mut world);
    assert!(
        written.contains("0.000000mm rotate 90 size 1.000000mm x 3.000000mm"),
        "the design written out does not say the turn:\n{written}"
    );
    let (_, again) = footprint_of(&written);
    assert_eq!(
        again.pads[0].rotation,
        Rotation::from_degrees(90.0),
        "the design written out reads back as a different pad"
    );
    assert_eq!(again.pads[0].size, (Nm::from_mm(1.0), Nm::from_mm(3.0)));
    assert_eq!(
        again.pads[0].slot,
        Some((Nm::from_mm(0.8), Nm::from_mm(2.0)))
    );
}

#[test]
fn a_pad_that_stands_square_is_written_as_it_always_was() {
    let (mut world, _) = footprint_of(&design_with(""));
    let written = cypcb_world::dsl::board_as_dsl(&mut world);
    let pad_line = written
        .lines()
        .find(|line| line.trim_start().starts_with("pad 1 "))
        .expect("the written design has the pad");
    assert!(
        !pad_line.contains("rotate"),
        "a square pad was written with a turn: {pad_line}"
    );
}
