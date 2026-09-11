//! Is copper that came out of a file treated as a person's or as the router's?
//!
//! `cargo test -p cypcb-kicad --test hand_drawn_copper_is_not_the_routers -- --nocapture`
//!
//! `apply_routes` marks everything it spawns `TraceSource::Autorouted`, and the
//! first act of the next run is to delete every trace that is `Autorouted` and
//! not locked. Two callers used it to materialise copper that came out of a
//! `.kicad_pcb` rather than out of this router:
//! `crates/cypcb-cli/src/board_source.rs`, which every command loads a board
//! through, and `crates/cypcb-cli/src/commands/from_kicad.rs`.
//!
//! So a straight segment a person drew in KiCad was imported as machine-made
//! and deleted by the first routing pass, while an arc from the same board
//! survived - `parse_track_arc` spawns a `Trace` directly and calls it
//! `Manual`. Measured before the fix on the fixture below: the arc came back
//! `Manual` in twelve segments, the straight segment came back `Autorouted`.
//!
//! **What this fixture can and cannot prove.** It was written by hand here, so
//! it is evidence about this importer and not about what KiCad emits. That
//! limit is why it carries no optional tokens at all: no `(locked)`, no via
//! with an unusual ring. A `(segment ...)` and an `(arc ...)` with the fields
//! both forms always carry is the whole of it, and the asymmetry under test
//! needs no token to be read - it comes from which code path spawns the entity.
//! Checking that this importer reads a lock the way real KiCad writes one needs
//! a file real KiCad wrote, and this project has none.

use std::path::{Path, PathBuf};

use cypcb_world::components::trace::{Trace, TraceSource};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/hand/one_straight_one_curved.kicad_pcb")
}

/// The two lines both import call sites use, in the order they use them.
fn imported() -> Vec<Trace> {
    let parsed = cypcb_kicad::parse_kicad_pcb(&fixture()).expect("the fixture parses");
    let mut world = parsed.world;
    if let Some(routes) = parsed.reference_routes {
        cypcb_router::apply_routes_as(&mut world, &routes, TraceSource::Manual);
    }
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Trace>();
    query.iter(ecs).cloned().collect()
}

#[test]
fn a_straight_run_and_a_curved_one_come_back_the_same_kind_of_copper() {
    let traces = imported();
    for trace in &traces {
        println!(
            "{:>2} segments, source {:?}, locked {}",
            trace.segments.len(),
            trace.source,
            trace.locked
        );
    }

    // The control first: this fixture has to produce both shapes, or the
    // assertion below is about one of them and says nothing about the other.
    // The arc arrives as the chords it stands for, so the two are told apart by
    // segment count rather than by order.
    assert_eq!(traces.len(), 2, "one straight run and one arc");
    assert!(
        traces.iter().any(|trace| trace.segments.len() == 1),
        "the straight segment has to arrive as one segment"
    );
    assert!(
        traces.iter().any(|trace| trace.segments.len() > 1),
        "the arc has to arrive as the chords it stands for"
    );

    for trace in &traces {
        assert_eq!(
            trace.source,
            TraceSource::Manual,
            "copper that came out of a file is a person's until something says \
             otherwise, and this one arrived as {:?} in {} segment(s)",
            trace.source,
            trace.segments.len()
        );
    }
}

#[test]
fn the_router_leaves_both_of_them_alone() {
    // The consequence, which is the reason the field matters at all. Routing a
    // board begins by deleting every trace the router thinks it drew, so
    // marking a person's copper `Autorouted` is what destroys it - not the mark
    // itself.
    let parsed = cypcb_kicad::parse_kicad_pcb(&fixture()).expect("the fixture parses");
    let mut world = parsed.world;
    let routes = parsed.reference_routes.expect("the fixture carries copper");
    cypcb_router::apply_routes_as(&mut world, &routes, TraceSource::Manual);

    let before = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).count()
    };

    // A second pass carrying nothing new. Whatever survives it is copper the
    // router agrees is not its own.
    let empty = cypcb_router::types::RoutingResult::complete(Vec::new(), Vec::new());
    cypcb_router::apply_routes(&mut world, &empty);

    let after = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).count()
    };
    println!("{before} traces before a routing pass, {after} after");

    assert_eq!(
        before, 2,
        "the fixture has to put two runs of copper on the board first"
    );
    assert_eq!(
        after, before,
        "a routing pass deleted copper that came out of the file"
    );
}

#[test]
fn the_same_copper_marked_as_the_routers_own_is_deleted() {
    // The control that gives the test above its meaning. If a routing pass
    // deleted nothing at all, "both survive" would hold for a reason that has
    // nothing to do with the mark - so the same fixture, imported with the mark
    // the old code used, has to lose its copper.
    let parsed = cypcb_kicad::parse_kicad_pcb(&fixture()).expect("the fixture parses");
    let mut world = parsed.world;
    let routes = parsed.reference_routes.expect("the fixture carries copper");
    cypcb_router::apply_routes_as(&mut world, &routes, TraceSource::Autorouted);

    let empty = cypcb_router::types::RoutingResult::complete(Vec::new(), Vec::new());
    cypcb_router::apply_routes(&mut world, &empty);

    let survivors: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };
    for trace in &survivors {
        println!(
            "marked as the router's own: {} segment(s) survived, source {:?}",
            trace.segments.len(),
            trace.source
        );
    }

    // One, not none, and which one is the whole finding. The arc never passes
    // through this function at all: `parse_track_arc` spawns its `Trace` while
    // the file is being read and calls it `Manual` there, so it was never at
    // risk whatever `apply_routes` was told. Only the straight segment goes
    // through the intermediate `RouteSegment`, and only the straight segment
    // was being deleted.
    assert_eq!(
        survivors.len(),
        1,
        "a routing pass has to delete copper the router believes it drew, or \
         the test above proves nothing about the mark"
    );
    assert!(
        survivors[0].segments.len() > 1,
        "the survivor has to be the arc, which never went through this \
         function; if the straight run is what survived, the asymmetry is \
         somewhere else than this test says"
    );
}
