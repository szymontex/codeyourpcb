//! `cypcb route` appends to a copy of the source, so it writes only its own copper.
//!
//! `cargo test -p cypcb-world --test only_the_copper_the_router_drew_is_written`
//!
//! The rule this pins: copper already on a net is kept and left alone. The
//! file the router writes starts as a copy of the source, so everything the
//! source declared is in it before the writer adds a character. Writing the
//! whole world on top of that put every hand-drawn trace in the file twice -
//! `cypcb route` reported no violations because the world it measured held one
//! copy, and `cypcb check` reported three on the same board because the file
//! held two and copper drawn over itself is a defect the checker knows.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::components::Layer;
use cypcb_world::dsl::{routed_traces_as_dsl, traces_as_dsl};
use cypcb_world::BoardWorld;

/// A board carrying one trace per net: VCC drawn by hand, GND by the router.
///
/// Both are in the world at the moment the writer runs, which is the state
/// `apply_routes` leaves behind: it despawns every autorouted trace before it
/// spawns this run's, and never touches a trace the source declared.
fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);

    let hand = world.intern_net("VCC");
    let mut manual = Trace::new(hand);
    manual.layer = Layer::TopCopper;
    manual.source = TraceSource::Manual;
    manual.segments.push(TraceSegment::new(
        Point::from_mm(2.0, 2.0),
        Point::from_mm(12.0, 2.0),
    ));
    world.spawn_entity((manual, hand));

    let routed = world.intern_net("GND");
    let mut laid = Trace::new(routed);
    laid.layer = Layer::TopCopper;
    laid.source = TraceSource::Autorouted;
    laid.segments.push(TraceSegment::new(
        Point::from_mm(2.0, 8.0),
        Point::from_mm(12.0, 8.0),
    ));
    world.spawn_entity((laid, routed));

    world
}

#[test]
fn the_copper_the_source_drew_is_not_written_again() {
    let mut world = board();
    let written = routed_traces_as_dsl(&mut world);

    assert!(
        !written.contains("trace VCC"),
        "the hand-drawn trace is already in the file this is appended to:\n{written}"
    );
    // The positive control: a writer that returned nothing at all would pass
    // the assertion above and produce a board with no routing in it.
    assert!(
        written.contains("trace GND"),
        "the copper the router laid has to be written:\n{written}"
    );
}

#[test]
fn the_writer_that_saves_a_whole_board_still_writes_all_of_it() {
    // The viewer saves the world rather than appending to a source, so it asks
    // the other writer and has to keep getting both traces.
    let mut world = board();
    let written = traces_as_dsl(&mut world);

    assert!(written.contains("trace VCC"), "{written}");
    assert!(written.contains("trace GND"), "{written}");
}

#[test]
fn a_via_the_source_locked_is_not_written_again_and_the_routers_is() {
    // `apply_routes` despawns every unlocked via and spawns its own unlocked,
    // so after it has run a locked via can only have come from the source -
    // where `trace GND { locked, via ... }` puts one - and an unlocked one can
    // only be this run's.
    let mut world = board();
    let net = world.intern_net("GND");

    let mut declared = Via::new(Point::from_mm(5.0, 5.0), net);
    declared.locked = true;
    world.spawn_entity((declared, net));

    let mine = Via::new(Point::from_mm(9.0, 9.0), net);
    world.spawn_entity((mine, net));

    let written = routed_traces_as_dsl(&mut world);

    assert!(
        !written.contains("via 5.000000mm,5.000000mm"),
        "the locked via is already in the file this is appended to:\n{written}"
    );
    assert!(
        written.contains("via 9.000000mm,9.000000mm"),
        "the via the router placed has to be written:\n{written}"
    );
}
