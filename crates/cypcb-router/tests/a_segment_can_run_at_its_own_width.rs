//! A trace is not one width end to end, and `apply_routes` used to make it one.
//!
//! `cargo test -p cypcb-router --test a_segment_can_run_at_its_own_width`
//!
//! One `Trace` is built per net and layer, and its width came from the first
//! route segment the grouping happened to see - `trace_widths.entry(key)
//! .or_insert(segment.width)`. Every other width in that group was dropped.
//!
//! Nothing in this repository showed it. Every benchmark fixture routes at one
//! width per net, and the only routed KiCad boards checked in carry one width
//! per net and layer too, so no fixture could fail. The loss is in the code
//! path rather than in the data, which is exactly the kind a fixture-driven
//! suite never finds: `PcbEngine::load_source` sends a file's own copper
//! through this function, so a board with a fattened power run or a neck into
//! a pad opened in the editor as uniform copper the file does not have.

use cypcb_core::{Nm, Point};
use cypcb_router::apply_routes;
use cypcb_router::types::{RouteSegment, RoutingResult};
use cypcb_world::components::trace::Trace;
use cypcb_world::{BoardWorld, Layer, NetId};

/// Two segments of one net on one layer, at two widths, joined end to end.
fn two_widths() -> RoutingResult {
    let net = NetId::new(0);
    RoutingResult::complete(
        vec![
            RouteSegment::new(
                net,
                Layer::TopCopper,
                Nm::from_mm(2.0),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            ),
            RouteSegment::new(
                net,
                Layer::TopCopper,
                Nm::from_mm(0.8),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(14.0, 0.0),
            ),
        ],
        Vec::new(),
    )
}

fn traces(world: &mut BoardWorld) -> Vec<Trace> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Trace>();
    query.iter(ecs).cloned().collect()
}

#[test]
fn a_narrower_segment_keeps_its_width() {
    let mut world = BoardWorld::new();
    apply_routes(&mut world, &two_widths());
    let traces = traces(&mut world);

    assert_eq!(traces.len(), 1, "one net on one layer is one trace");
    let trace = &traces[0];
    assert_eq!(
        trace.segments.len(),
        2,
        "both segments should be on the trace"
    );

    assert_eq!(
        trace.width_at(0),
        Nm::from_mm(2.0),
        "the first segment runs at the width the group was built from"
    );
    assert_eq!(
        trace.width_at(1),
        Nm::from_mm(0.8),
        "the second segment is 0.8mm in the routing result and has to stay 0.8mm; \
         a trace that reports 2.0mm here is copper the router never laid"
    );
}

#[test]
fn the_necked_stretch_is_measurable() {
    let mut world = BoardWorld::new();
    apply_routes(&mut world, &two_widths());
    let traces = traces(&mut world);

    // The number `neck 0.8mm for 4mm` is a claim about. `NeckDownRule` can
    // only check that the declaration is coherent while this is unavailable.
    assert_eq!(
        traces[0].necked_length(),
        Nm::from_mm(4.0),
        "4mm of the 14mm runs narrower than the trace"
    );
}

#[test]
fn a_trace_of_one_width_still_says_it_once() {
    // The common case has to be untouched: every segment inherits, and nothing
    // carries a redundant copy of the trace's own width. A serialised board
    // that started writing a width per segment would be a diff on every file
    // for no change in meaning.
    let net = NetId::new(0);
    let uniform = RoutingResult::complete(
        vec![
            RouteSegment::new(
                net,
                Layer::TopCopper,
                Nm::from_mm(0.25),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            ),
            RouteSegment::new(
                net,
                Layer::TopCopper,
                Nm::from_mm(0.25),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(20.0, 0.0),
            ),
        ],
        Vec::new(),
    );

    let mut world = BoardWorld::new();
    apply_routes(&mut world, &uniform);
    let traces = traces(&mut world);

    assert_eq!(traces[0].width, Nm::from_mm(0.25));
    assert!(
        traces[0].segments.iter().all(|s| s.width.is_none()),
        "a uniform trace states its width once, on the trace"
    );
    assert_eq!(
        traces[0].necked_length(),
        Nm::ZERO,
        "nothing is necked here"
    );
}

#[test]
fn a_wider_segment_is_not_a_neck() {
    // The group's width is the first segment's, so a run that starts thin and
    // fattens leaves the *wide* segment carrying its own width. `necked_length`
    // has to exclude it: a trace that reports 10mm of neck here would fail a
    // `neck ... for 4mm` declaration that the copper actually keeps.
    //
    // This case is why the filter compares against the trace's width instead
    // of asking whether a segment states one at all. The first draft of these
    // tests could not tell those two apart - both answered 4mm on a board
    // whose only stated segment was the narrow one - so a mutation swapping
    // them survived.
    let net = NetId::new(0);
    let thin_first = RoutingResult::complete(
        vec![
            RouteSegment::new(
                net,
                Layer::TopCopper,
                Nm::from_mm(0.8),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(4.0, 0.0),
            ),
            RouteSegment::new(
                net,
                Layer::TopCopper,
                Nm::from_mm(2.0),
                Point::from_mm(4.0, 0.0),
                Point::from_mm(14.0, 0.0),
            ),
        ],
        Vec::new(),
    );

    let mut world = BoardWorld::new();
    apply_routes(&mut world, &thin_first);
    let traces = traces(&mut world);

    assert_eq!(
        traces[0].width,
        Nm::from_mm(0.8),
        "the group takes the first"
    );
    assert_eq!(traces[0].width_at(1), Nm::from_mm(2.0));
    assert_eq!(
        traces[0].necked_length(),
        Nm::ZERO,
        "nothing here is narrower than the trace, so nothing is necked"
    );
}
