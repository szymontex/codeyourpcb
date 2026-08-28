//! A saved board keeps its curve.
//!
//! `cargo test -p cypcb-world --test a_curve_survives_being_written_down`
//!
//! Stage three of row 2 of the KiCad parity audit. The model holds straight
//! segments because everything that measures copper measures straight
//! segments, and until now the writer wrote those back: a design that stated
//! one `arc` came home as a dozen `path` points, checked the same and read
//! worse, and the next save flattened the flattening.
//!
//! The fix is the shape the stitched vias already use - a marker recording
//! what a run of copper came from - so this test is about a file, not a shape:
//! read a design, write it, read what was written.

use cypcb_core::Point;
use cypcb_world::components::trace::{Curve, Trace};
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

/// Every point of copper on the board, sorted, so two boards can be compared
/// without depending on the order their traces come out of the world in.
fn copper(world: &mut BoardWorld) -> Vec<(i64, i64, i64, i64)> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Trace>();
    let mut found: Vec<(i64, i64, i64, i64)> = query
        .iter(ecs)
        .flat_map(|trace| {
            trace
                .segments
                .iter()
                .map(|segment| {
                    (
                        segment.start.x.0,
                        segment.start.y.0,
                        segment.end.x.0,
                        segment.end.y.0,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    found.sort_unstable();
    found
}

fn curves(world: &mut BoardWorld) -> Vec<Curve> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Curve>();
    let mut found: Vec<Curve> = query.iter(ecs).copied().collect();
    found.sort_by_key(|curve| (curve.centre.x.0, curve.centre.y.0, curve.sweep_millideg));
    found
}

const CURVED: &str = "version 1\n\nboard b {\n    size 24mm x 20mm\n    layers 2\n}\n\nnet SIG {\n}\n\ntrace SIG {\n    layer top\n    width 0.25mm\n    path 8mm, 6mm -> 12mm, 6mm\n    arc centre 12mm, 10mm sweep 90 clockwise\n    path 8mm, 10mm -> 8mm, 14mm\n}\n";

#[test]
fn the_saved_file_still_says_arc() {
    let mut world = world_of(CURVED);
    let written = board_as_dsl(&mut world);

    assert!(
        written.contains("arc start "),
        "the curve is written as a curve:\n{written}"
    );
    assert!(
        written.contains("centre 12.000000mm,10.000000mm")
            && written.contains("sweep 90 clockwise"),
        "with the centre it turns about and the way it turns:\n{written}"
    );
    // Twelve chords would be twelve points in a `path`. The whole point of the
    // marker is that they are not there.
    assert!(
        !written.contains("12.023mm"),
        "and not as the chords it was flattened into:\n{written}"
    );
}

#[test]
fn the_copper_is_the_same_copper_after_a_round_trip() {
    let mut first = world_of(CURVED);
    let written = board_as_dsl(&mut first);
    let mut second = world_of(&written);

    assert_eq!(
        copper(&mut second),
        copper(&mut first),
        "a save and a reload draw the same copper, chord for chord"
    );
    assert_eq!(
        curves(&mut second),
        curves(&mut first),
        "and the curve behind it is the same curve"
    );
}

#[test]
fn saving_twice_changes_nothing() {
    // The failure this replaces: each trip through the writer turned a curve
    // into chords, and the trip after that flattened those again. A file that
    // is stable under saving is a file a person can keep working in.
    let mut first = world_of(CURVED);
    let once = board_as_dsl(&mut first);
    let mut second = world_of(&once);
    let twice = board_as_dsl(&mut second);

    assert_eq!(once, twice, "the second save writes exactly the first file");
}

#[test]
fn the_written_curve_states_where_it_begins() {
    // A person writing an arc leaves the start to the copper in front of it.
    // A file a tool wrote cannot: the traces come out of the world in
    // archetype order, so a curve that relied on what was written before it
    // would move the moment anything else about the board changed.
    let mut world = world_of(CURVED);
    let written = board_as_dsl(&mut world);
    assert!(
        written.contains("arc start 12.000000mm,6.000000mm"),
        "the saved curve says where it starts:\n{written}"
    );

    // And what it says is where the straight run before it ended.
    let start = Point::from_mm(12.0, 6.0);
    let mut query = world.ecs_mut().query::<(&Trace, Option<&Curve>)>();
    let ends: Vec<Point> = query
        .iter(world.ecs())
        .filter(|(_, curve)| curve.is_none())
        .filter_map(|(trace, _)| trace.segments.last().map(|segment| segment.end))
        .collect();
    assert!(
        ends.contains(&start),
        "a straight run really does end there: {ends:?}"
    );
}

#[test]
fn a_stated_start_beats_the_copper_in_front_of_it() {
    // The two ways of saying where a curve begins have to be ranked, and the
    // explicit one wins: a file that states a start means it, and silently
    // preferring the copper before it would move the curve on a board where
    // both are present.
    let source = "version 1\n\nboard b {\n    size 24mm x 20mm\n    layers 2\n}\n\nnet SIG {\n}\n\ntrace SIG {\n    layer top\n    width 0.25mm\n    path 4mm, 4mm -> 6mm, 4mm\n    arc start 12mm, 6mm centre 12mm, 10mm sweep 90 clockwise\n}\n";
    let mut world = world_of(source);

    let mut query = world.ecs_mut().query::<(&Trace, &Curve)>();
    let starts: Vec<Point> = query
        .iter(world.ecs())
        .filter_map(|(trace, _)| trace.segments.first().map(|segment| segment.start))
        .collect();
    assert_eq!(
        starts,
        vec![Point::from_mm(12.0, 6.0)],
        "the curve begins where it says, not where the path before it ended"
    );
}

#[test]
fn a_nets_blocks_come_out_in_an_order_of_their_own() {
    // Bevy iterates archetypes, and a trace carrying a curve sits in a
    // different archetype from a plain one - so the order blocks are written
    // in follows what was spawned first rather than anything about the board.
    // Here the curve is spawned first and still has to be written last,
    // because its copper starts further along than the straight run's.
    let source = "version 1\n\nboard b {\n    size 24mm x 20mm\n    layers 2\n}\n\nnet SIG {\n}\n\ntrace SIG {\n    layer top\n    width 0.25mm\n    arc start 12mm, 6mm centre 12mm, 10mm sweep 90 clockwise\n}\n\ntrace SIG {\n    layer top\n    width 0.25mm\n    path 8mm, 6mm -> 12mm, 6mm\n}\n";
    let mut world = world_of(source);
    let written = board_as_dsl(&mut world);

    let path_at = written
        .find("path 8.000000mm,6.000000mm")
        .expect("the straight run is in the file");
    let arc_at = written.find("arc start").expect("and so is the curve");
    assert!(
        path_at < arc_at,
        "the straight run starts first, so it is written first:\n{written}"
    );
}
