//! `board { teardrops }`: the design says it, not the command line.
//!
//! `cargo test -p cypcb-world --test the_board_can_ask_for_teardrops`
//!
//! A flag on `cypcb export` is a person's choice on one run. Where a board
//! states what it is - the fabricator, the stackup, the finish - is the board
//! block, and teardrops belong beside them: the fillet is part of what the
//! design asks a house to make, not part of how somebody exported it today.

use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Read this source into a world, the way every other reader does.
fn world_of(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(
        !parsed.has_errors(),
        "the source parses: {:?}",
        parsed.errors
    );
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(
        result.errors.is_empty(),
        "the design syncs: {:?}",
        result.errors
    );
    world
}

const BARE: &str = "version 1\n\nboard b {\n    size 20mm x 10mm\n    layers 2\n    teardrops\n}\n";

const WITH_RATIOS: &str = "version 1\n\nboard b {\n    size 20mm x 10mm\n    layers 2\n    teardrops {\n        length 0.25\n        width 0.75\n    }\n}\n";

#[test]
fn a_board_that_says_nothing_asks_for_nothing() {
    let world = world_of("version 1\n\nboard b {\n    size 20mm x 10mm\n    layers 2\n}\n");
    assert!(
        world.teardrops().is_none(),
        "silence is not a request: a board exported yesterday keeps its copper"
    );
}

#[test]
fn the_bare_word_asks_for_the_ordinary_fillet() {
    let world = world_of(BARE);
    let asked = world.teardrops().expect("the board asked for teardrops");
    assert_eq!(asked, cypcb_world::components::Teardrops::default());
}

#[test]
fn the_block_states_its_own_ratios() {
    let world = world_of(WITH_RATIOS);
    let asked = world.teardrops().expect("the board asked for teardrops");
    assert!(
        (asked.length - 0.25).abs() < 1e-9 && (asked.width - 0.75).abs() < 1e-9,
        "the ratios are the ones written: {asked:?}"
    );
}

#[test]
fn the_writer_says_it_back_the_way_it_was_asked() {
    // A board that asked with the ordinary ratios gets the bare word back.
    // Writing the numbers would turn a request into a specification the source
    // never made, and the next reader could not tell the two apart.
    let mut world = world_of(BARE);
    let written = board_as_dsl(&mut world);
    assert!(
        written.contains("    teardrops\n"),
        "the bare word comes back bare:\n{written}"
    );

    let mut world = world_of(WITH_RATIOS);
    let written = board_as_dsl(&mut world);
    assert!(
        written.contains("teardrops {") && written.contains("length 0.25"),
        "the ratios come back as they were written:\n{written}"
    );

    // And the round trip closes: what the writer produced reads back the same.
    let again = world_of(&written);
    let asked = again.teardrops().expect("the written board still asks");
    assert!(
        (asked.length - 0.25).abs() < 1e-9,
        "a second reading agrees with the first: {asked:?}"
    );
}
