//! `board b { fab oshpark }` has to survive being written down.
//!
//! `cargo test -p cypcb-world --test a_board_says_which_fab_it_is_for`
//!
//! The DSL is this project's storage format, so a fact the language can state
//! and the writer drops is a fact that disappears the first time a board is
//! imported and written back. The fab is the table every clearance in the
//! design was checked against, which makes losing it quietly worse than most.

use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Read a design the way the CLI does.
fn read(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let sync = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(sync.errors.is_empty(), "{:?}", sync.errors);
    world
}

fn board(fab_line: &str) -> String {
    format!(
        "version 1\n\n\
         board t {{\n    size 30mm x 20mm\n    layers 2\n{fab_line}}}\n\n\
         component R1 resistor \"0402\" {{\n    value \"10k\"\n    at 10mm, 10mm\n}}\n"
    )
}

#[test]
fn the_fab_reaches_the_board_model() {
    let world = read(&board("    fab oshpark\n"));
    assert_eq!(world.fab(), Some("oshpark"));
}

#[test]
fn a_board_that_names_no_fab_says_none() {
    let world = read(&board(""));
    assert_eq!(
        world.fab(),
        None,
        "silence is not the same as naming this project's default out loud"
    );
}

#[test]
fn the_fab_survives_being_written_back_out() {
    let mut world = read(&board("    fab oshpark\n"));
    let written = board_as_dsl(&mut world);

    assert!(
        written.contains("fab oshpark"),
        "the writer dropped the fab:\n{written}"
    );

    // Round trip: what came out has to read back as the same fact, which is a
    // stronger claim than the string appearing somewhere in the output.
    let again = read(&written);
    assert_eq!(again.fab(), Some("oshpark"));
}

#[test]
fn a_board_with_no_fab_writes_no_fab() {
    let mut world = read(&board(""));
    let written = board_as_dsl(&mut world);

    assert!(
        !written.contains("fab "),
        "a board that named no fab must not have one invented for it:\n{written}"
    );
}
