//! Copper on no net is not put on a net by the files that name nets.
//!
//! `cargo test -p cypcb-export --test copper_on_no_net_is_not_put_on_a_net`
//!
//! A trace or via with `net_id: None` is on no net - KiCad's net 0. IPC-356
//! lists test points per net, and a via on no net is not a point a tester has
//! anything to compare against, as a pad on no net is not (the comment in
//! `ipc356.rs` where pads are read says so). IPC-2581 writes a copper set per
//! net, and copper on no net goes in the set it already uses for a pour on
//! none, `unknown`, rather than in the first net's.

use cypcb_core::{Nm, Point};
use cypcb_export::ipc2581::{export_ipc2581, HouseTolerances};
use cypcb_export::ipc356::export_ipc356;
use cypcb_world::components::trace::{Trace, TraceSegment, Via};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, NetId};

fn board(trace_net: Option<NetId>, via_net: Option<NetId>) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    // The first net on the board, which copper on no net used to be put on.
    world.intern_net("FIRST");
    let mut trace = Trace::new(NetId::new(0));
    trace.net_id = trace_net;
    trace.add_segment(TraceSegment::new(
        Point::from_mm(10.0, 10.0),
        Point::from_mm(20.0, 10.0),
    ));
    world.spawn_entity((trace,));
    let mut via = Via::new(Point::from_mm(15.0, 15.0), NetId::new(0));
    via.net_id = via_net;
    world.spawn_entity((via,));
    world
}

fn netlist_vias(world: &mut BoardWorld) -> Vec<String> {
    let (text, _) = export_ipc356(world, &FootprintLibrary::new(), "t");
    text.lines()
        .filter(|l| l.contains("VIA"))
        .map(str::to_string)
        .collect()
}

fn ipc2581(world: &mut BoardWorld) -> String {
    export_ipc2581(
        world,
        &FootprintLibrary::new(),
        HouseTolerances::default(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .0
}

#[test]
fn a_via_on_no_net_is_not_a_test_point() {
    let on_a_net = netlist_vias(&mut board(None, Some(NetId::new(0))));
    assert_eq!(on_a_net.len(), 1, "the control: {on_a_net:?}");
    assert!(on_a_net[0].contains("FIRST"), "{on_a_net:?}");

    let on_none = netlist_vias(&mut board(None, None));
    assert!(on_none.is_empty(), "{on_none:?}");
}

#[test]
fn a_trace_on_no_net_is_a_set_of_its_own_in_ipc2581() {
    let on_a_net = ipc2581(&mut board(Some(NetId::new(0)), None));
    assert!(on_a_net.contains("<Set net=\"FIRST\">"), "the control");
    assert!(!on_a_net.contains("net=\"unknown\""));

    let on_none = ipc2581(&mut board(None, None));
    assert!(on_none.contains("<Set net=\"unknown\">"));
    assert!(!on_none.contains("<Set net=\"FIRST\">"));
}
