//! `arc centre X,Y sweep 90`: copper that turns a corner.
//!
//! `cargo test -p cypcb-world --test copper_can_turn_a_corner`
//!
//! Stage two of row 2 of the KiCad parity audit. Stage one put a curve in the
//! model and proved the checker can measure one; this is the language saying
//! it. An arc continues from wherever the copper already is and states the
//! centre it turns about and how far - which is what the model holds, so
//! nothing is converted and no convention is invented.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Trace;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Sync a design, and hand back the world with whatever errors came out.
fn synced(source: &str) -> (BoardWorld, Vec<String>) {
    let parsed = cypcb_parser::parse(source);
    assert!(
        !parsed.has_errors(),
        "the source parses: {:?}",
        parsed.errors
    );
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    let errors = result.errors.iter().map(|e| e.to_string()).collect();
    (world, errors)
}

fn world_of(source: &str) -> BoardWorld {
    let (world, errors) = synced(source);
    assert!(errors.is_empty(), "the design syncs: {errors:?}");
    world
}

/// Every trace's segments, in the order they were spawned.
fn traces(world: &mut BoardWorld) -> Vec<Trace> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Trace>();
    query.iter(ecs).cloned().collect()
}

const HEAD: &str =
    "version 1\n\nboard b {\n    size 24mm x 20mm\n    layers 2\n}\n\nnet SIG {\n}\n\n";

/// A track up the board, a quarter turn to the left, and away.
fn curved(direction: &str) -> String {
    format!(
        "{HEAD}trace SIG {{\n    layer top\n    width 0.25mm\n    path 8mm, 6mm -> 12mm, 6mm\n    \
         arc centre 12mm, 10mm sweep 90{direction}\n}}\n"
    )
}

#[test]
fn the_curve_starts_where_the_copper_stopped() {
    // An arc states no start of its own. A curve that began somewhere else
    // would be a second trace beside the first with a gap between them, which
    // is an open circuit that checks clean.
    let mut world = world_of(&curved(" clockwise"));
    let traces = traces(&mut world);
    assert_eq!(traces.len(), 2, "the path and the arc are each copper");

    let path = &traces[0];
    let arc = &traces[1];
    assert_eq!(
        path.segments.last().expect("the path has copper").end,
        arc.segments.first().expect("the arc has copper").start,
        "the curve picks up exactly where the straight run left off"
    );
    // A quarter turn clockwise about 12mm,10mm from due south lands due west.
    let end = arc.segments.last().expect("the arc has copper").end;
    assert!(
        (end.x.0 - Nm::from_mm(8.0).0).abs() <= 1_000
            && (end.y.0 - Nm::from_mm(10.0).0).abs() <= 1_000,
        "and it ends where the geometry says: {end:?}"
    );
}

#[test]
fn which_way_it_turns_is_the_word_beside_the_sweep() {
    // Angles grow counter-clockwise, so that is the direction with no word on
    // it. A tool that dropped the word would send the copper the long way
    // round the board.
    let mut widdershins = world_of(&curved(""));
    let mut clockwise = world_of(&curved(" clockwise"));

    let anti_end = traces(&mut widdershins)[1]
        .segments
        .last()
        .expect("copper")
        .end;
    let clock_end = traces(&mut clockwise)[1]
        .segments
        .last()
        .expect("copper")
        .end;

    assert!(
        (anti_end.x.0 - Nm::from_mm(16.0).0).abs() <= 1_000,
        "counter-clockwise from due south comes round to due east: {anti_end:?}"
    );
    assert!(
        (clock_end.x.0 - Nm::from_mm(8.0).0).abs() <= 1_000,
        "and clockwise goes the other way: {clock_end:?}"
    );
}

#[test]
fn the_copper_is_chords_and_all_of_them_are_on_the_curve() {
    // What reaches the board is the flattening: everything downstream - the
    // checker, the router, both exporters - measures straight segments.
    let mut world = world_of(&curved(" clockwise"));
    let traces = traces(&mut world);
    let arc = &traces[1];

    assert!(
        arc.segments.len() >= 8,
        "a quarter turn at 4mm radius is a dozen chords, not one: {}",
        arc.segments.len()
    );

    let centre = Point::from_mm(12.0, 10.0);
    for segment in &arc.segments {
        for point in [segment.start, segment.end] {
            let dx = (point.x.0 - centre.x.0) as f64;
            let dy = (point.y.0 - centre.y.0) as f64;
            let radius = (dx * dx + dy * dy).sqrt();
            assert!(
                (radius - Nm::from_mm(4.0).0 as f64).abs() <= 1_000.0,
                "every chord end sits on the circle: {radius} against 4000000"
            );
        }
    }
}

#[test]
fn an_arc_with_nothing_to_continue_from_is_an_error() {
    // Silence here would be a trace that quietly lost its curve, and a board
    // that goes out with a connection nobody drew.
    let source = format!(
        "{HEAD}trace SIG {{\n    layer top\n    width 0.25mm\n    \
         arc centre 12mm, 10mm sweep 90\n}}\n"
    );
    let (_world, errors) = synced(&source);
    assert_eq!(
        errors.len(),
        1,
        "an arc that starts nowhere has to be reported: {errors:?}"
    );
    assert!(
        errors[0].contains("arc"),
        "and the message says what it is about: {}",
        errors[0]
    );
}

#[test]
fn the_run_carries_on_after_the_curve() {
    // A path after an arc starts where the arc ended, so a track can turn
    // twice - which is what a real board does at every corner.
    let source = format!(
        "{HEAD}trace SIG {{\n    layer top\n    width 0.25mm\n    path 8mm, 6mm -> 12mm, 6mm\n    \
         arc centre 12mm, 10mm sweep 90 clockwise\n    path 8mm, 10mm -> 8mm, 14mm\n}}\n"
    );
    let mut world = world_of(&source);
    let traces = traces(&mut world);
    assert_eq!(traces.len(), 3, "two straight runs and the curve between");
    assert_eq!(
        traces[1].segments.last().expect("copper").end,
        traces[2].segments.first().expect("copper").start,
        "the second straight run picks up where the curve left off"
    );
}
