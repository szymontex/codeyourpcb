//! Copper the router lays gets the neck the net declared.
//!
//! `cargo test -p cypcb-router --test a_routed_net_gets_the_neck_it_asked_for`
//!
//! A net could state `neck 0.8mm for 4mm` and the router laid full-width
//! copper: the search reads a net's width and knows nothing about necks. It
//! still does not, and deliberately - it plans on a grid of whole cells and a
//! 0.8mm neck on a 0.254mm cell is not expressible there, so teaching it would
//! move every routing measurement this project has. The declaration is applied
//! to the finished route instead, which is what `sync_ast_to_world` does to a
//! trace drawn in the language.
//!
//! One `Trace` holds every segment a net has on a layer, and a net with more
//! than two pads branches: the list is a set of chains. Necking the tail of
//! that vector would put thin copper wherever one branch happens to end, so
//! the neck is drawn per contiguous run.

use cypcb_core::{Nm, Point};
use cypcb_router::apply_routes;
use cypcb_router::types::{RouteSegment, RoutingResult};
use cypcb_world::components::trace::{Trace, TraceNeck};
use cypcb_world::registry::NetConstraints;
use cypcb_world::{BoardWorld, Layer, NetId};

/// A world whose net `SIG` declares `neck`, if one is given.
fn world_with(neck: Option<TraceNeck>) -> (BoardWorld, NetId) {
    let mut world = BoardWorld::new();
    let net = world.intern_net("SIG");
    if let Some(neck) = neck {
        world.set_net_constraints(
            net,
            NetConstraints {
                neck: Some(neck),
                ..NetConstraints::default()
            },
        );
    }
    (world, net)
}

/// One straight 20mm run of 2mm copper.
fn one_run(net: NetId) -> RoutingResult {
    RoutingResult::complete(
        vec![RouteSegment::new(
            net,
            Layer::TopCopper,
            Nm::from_mm(2.0),
            Point::from_mm(0.0, 0.0),
            Point::from_mm(20.0, 0.0),
        )],
        Vec::new(),
    )
}

/// Two runs of the same net on the same layer, not joined: a branch.
fn two_runs(net: NetId) -> RoutingResult {
    RoutingResult::complete(
        vec![
            RouteSegment::new(
                net,
                Layer::TopCopper,
                Nm::from_mm(2.0),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(20.0, 0.0),
            ),
            RouteSegment::new(
                net,
                Layer::TopCopper,
                Nm::from_mm(2.0),
                Point::from_mm(0.0, 10.0),
                Point::from_mm(20.0, 10.0),
            ),
        ],
        Vec::new(),
    )
}

fn only_trace(world: &mut BoardWorld) -> Trace {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Trace>();
    let traces: Vec<Trace> = query.iter(ecs).cloned().collect();
    assert_eq!(traces.len(), 1, "one net on one layer is one trace");
    traces.into_iter().next().expect("the one trace")
}

const NECK: TraceNeck = TraceNeck {
    width: Nm(800_000),
    length: Nm(4_000_000),
};

#[test]
fn the_declared_neck_reaches_the_copper() {
    let (mut world, net) = world_with(Some(NECK));
    apply_routes(&mut world, &one_run(net));
    let trace = only_trace(&mut world);

    assert_eq!(
        trace.necked_length(),
        Nm::from_mm(4.0),
        "the net asked for 4mm of 0.8mm copper and the router laid 20mm of 2mm"
    );
    assert_eq!(trace.width_at(0), Nm::from_mm(2.0));
    assert_eq!(
        trace.width_at(trace.segments.len() - 1),
        Nm::from_mm(0.8),
        "the thin stretch is at the end of the run"
    );
}

#[test]
fn a_net_that_declares_nothing_is_routed_as_before() {
    // Every benchmark in this project is such a net. If this moves, every
    // routing measurement moves with it.
    let (mut world, net) = world_with(None);
    apply_routes(&mut world, &one_run(net));
    let trace = only_trace(&mut world);

    assert_eq!(trace.necked_length(), Nm::ZERO);
    assert_eq!(trace.segments.len(), 1, "nothing was cut");
    assert!(trace.segments.iter().all(|s| s.width.is_none()));
}

#[test]
fn each_branch_gets_its_own_neck_and_none_lands_mid_board() {
    // The case that makes `apply_neck` walk runs instead of the vector: two
    // chains on one net and layer. Necking the tail of the vector would leave
    // the first chain full width and put 4mm of thin copper at the end of the
    // second only.
    let (mut world, net) = world_with(Some(NECK));
    apply_routes(&mut world, &two_runs(net));
    let trace = only_trace(&mut world);

    assert_eq!(trace.runs().len(), 2, "two chains");
    assert_eq!(
        trace.necked_length(),
        Nm::from_mm(8.0),
        "each chain ends in 4mm of thin copper"
    );
    for range in trace.runs() {
        let last = range.end - 1;
        assert_eq!(
            trace.width_at(last),
            Nm::from_mm(0.8),
            "run ending at segment {last} should end thin"
        );
        assert_eq!(
            trace.width_at(range.start),
            Nm::from_mm(2.0),
            "and start wide"
        );
    }
}
