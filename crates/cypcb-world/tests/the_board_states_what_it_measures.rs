//! `dimension { from ... to ... offset ... }`: the board states what it measures.
//!
//! `cargo test -p cypcb-world --test the_board_states_what_it_measures`
//!
//! A fabricator receives copper, drills and an outline, and none of them says
//! how big the board is meant to be - they say how big it is. A dimension is
//! the design writing the measurement down so the two can be compared. Item 9
//! of the KiCad parity audit, the half the legend does not cover.

use cypcb_world::components::BoardDimension;
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

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

fn dimensions(world: &mut BoardWorld) -> Vec<BoardDimension> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&BoardDimension>();
    query.iter(ecs).copied().collect()
}

const MEASURED: &str = "version 1\n\nboard b {\n    size 40mm x 25mm\n    layers 2\n}\n\ndimension {\n    from 0mm, 0mm\n    to 40mm, 0mm\n    offset 3mm\n}\n";

const BARE: &str = "version 1\n\nboard b {\n    size 40mm x 25mm\n    layers 2\n}\n\ndimension {\n    from 0mm, 0mm\n    to 0mm, 25mm\n}\n";

#[test]
fn the_distance_is_computed_from_the_ends_rather_than_written_down() {
    // The file never states the number. If it did, a design could be edited
    // into saying 40mm about a 39mm gap, which is the one thing a dimension
    // exists to make impossible.
    let mut world = world_of(MEASURED);
    let measured = dimensions(&mut world);
    assert_eq!(measured.len(), 1, "one dimension was written, one is here");
    assert_eq!(
        measured[0].length(),
        cypcb_core::Nm(40_000_000),
        "40mm across, because that is what its two ends are"
    );
}

#[test]
fn a_dimension_that_says_nothing_about_offset_gets_a_usable_one() {
    // A line lying on the edge it measures is a line nobody can read.
    let mut world = world_of(BARE);
    let measured = dimensions(&mut world);
    assert_eq!(
        measured[0].offset,
        BoardDimension::DEFAULT_OFFSET,
        "the default stands the line off the board"
    );
    assert_eq!(
        BoardDimension::DEFAULT_OFFSET,
        cypcb_core::Nm(2_000_000),
        "and it is 2mm"
    );
}

#[test]
fn a_measurement_survives_being_written_down() {
    let mut world = world_of(MEASURED);
    let written = board_as_dsl(&mut world);

    assert!(
        written.contains("dimension {"),
        "the saved file still measures the board:\n{written}"
    );
    assert!(
        written.contains("from 0.000000mm, 0.000000mm")
            && written.contains("to 40.000000mm, 0.000000mm"),
        "with the same two ends:\n{written}"
    );
    assert!(
        written.contains("offset 3.000000mm"),
        "and the same offset, not the default:\n{written}"
    );

    // The real test of a writer: read what it wrote and get the same board.
    let mut again = world_of(&written);
    assert_eq!(
        dimensions(&mut again),
        dimensions(&mut world),
        "a save and a reload measure the same thing"
    );
}
