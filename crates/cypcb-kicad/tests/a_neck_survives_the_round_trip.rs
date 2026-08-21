//! A trace that runs thin into a pad has to come back thin.
//!
//! `cargo test -p cypcb-kicad --test a_neck_survives_the_round_trip`
//!
//! KiCad writes one `(width ...)` inside every `(segment ...)`, and this
//! project's board writer wrote the *trace's* width for all of them. A board
//! with a neck went out uniform: the copper the designer made thin on purpose
//! came back at full width, and the file said the board was manufacturable in
//! a way it is not.
//!
//! The other half of the same round trip was fixed first - `apply_routes` kept
//! only the first width per net and layer, so a file's own neck was lost on
//! the way *in*. Both halves have to hold or the trip is not a round one, and
//! testing them apart is what lets each say which end it is about.

use std::collections::BTreeSet;

use cypcb_core::{Nm, Point};
use cypcb_kicad::{parse_kicad_pcb, write_board};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::Layer;
use cypcb_world::BoardWorld;

/// A 2mm run that necks to 0.8mm for its last 4mm, on the top layer.
fn board_with_a_neck() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "neck".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        2,
    );
    let net = world.intern_net("PWR");

    world.ecs_mut().spawn((
        Trace {
            layer: Layer::TopCopper,
            width: Nm::from_mm(2.0),
            segments: vec![
                TraceSegment::new(Point::from_mm(4.0, 10.0), Point::from_mm(14.0, 10.0)),
                TraceSegment::new_with_width(
                    Point::from_mm(14.0, 10.0),
                    Point::from_mm(18.0, 10.0),
                    Nm::from_mm(0.8),
                ),
            ],
            net_id: net,
            locked: false,
            source: TraceSource::Manual,
        },
        net,
    ));
    world
}

/// Every `(width ...)` the file states inside a `(segment ...)`, in mm.
fn segment_widths(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.trim_start().starts_with("(segment "))
        .filter_map(|line| {
            let start = line.find("(width ")? + "(width ".len();
            let end = line[start..].find(')')? + start;
            Some(line[start..end].to_string())
        })
        .collect()
}

#[test]
fn the_file_states_a_width_per_segment() {
    let mut world = board_with_a_neck();
    let text = write_board(&mut world, "cypcb-test");
    let widths = segment_widths(&text);

    assert_eq!(widths.len(), 2, "two segments were written; got {widths:?}");
    let distinct: BTreeSet<&String> = widths.iter().collect();
    assert_eq!(
        distinct.len(),
        2,
        "the two segments run at different widths and the file has to say so; \
         it says {widths:?}"
    );
    assert!(
        widths.iter().any(|w| w.starts_with("0.8")),
        "the necked segment is 0.8mm; the file says {widths:?}"
    );
    assert!(
        widths.iter().any(|w| w.starts_with('2')),
        "the wide segment is 2mm; the file says {widths:?}"
    );
}

#[test]
fn reading_it_back_gives_the_same_two_widths() {
    let mut world = board_with_a_neck();
    let text = write_board(&mut world, "cypcb-test");

    let path = std::env::temp_dir().join("cypcb-neck-round-trip.kicad_pcb");
    std::fs::write(&path, &text).expect("write the board out");
    let parsed = parse_kicad_pcb(&path).expect("read it back");
    let _ = std::fs::remove_file(&path);

    let routes = parsed
        .reference_routes
        .expect("the board carries copper")
        .routes;
    let mut widths: Vec<i64> = routes.iter().map(|s| s.width.raw()).collect();
    widths.sort_unstable();

    assert_eq!(
        widths,
        vec![Nm::from_mm(0.8).raw(), Nm::from_mm(2.0).raw()],
        "the neck and the wide run both have to survive being read back"
    );
}

#[test]
fn a_uniform_trace_is_written_exactly_as_before() {
    // The common case must not move. Every segment inherits the trace's width,
    // so the file states the same number twice and nothing about it changed.
    let mut world = BoardWorld::new();
    world.set_board(
        "uniform".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        2,
    );
    let net = world.intern_net("SIG");
    world.ecs_mut().spawn((
        Trace {
            layer: Layer::TopCopper,
            width: Nm::from_mm(0.25),
            segments: vec![
                TraceSegment::new(Point::from_mm(4.0, 10.0), Point::from_mm(14.0, 10.0)),
                TraceSegment::new(Point::from_mm(14.0, 10.0), Point::from_mm(18.0, 10.0)),
            ],
            net_id: net,
            locked: false,
            source: TraceSource::Manual,
        },
        net,
    ));

    let text = write_board(&mut world, "cypcb-test");
    let widths = segment_widths(&text);
    assert_eq!(widths.len(), 2);
    assert_eq!(
        widths[0], widths[1],
        "a trace of one width writes one number; it wrote {widths:?}"
    );
    assert!(widths[0].starts_with("0.25"), "got {widths:?}");
}
