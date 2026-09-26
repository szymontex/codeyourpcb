//! What the clearance rule counts as one place.
//!
//! `cargo test -p cypcb-drc --test a_place_is_one_run_of_copper`
//!
//! One row per place where a trace comes too close to another net's copper,
//! and a place is one unbroken run of the trace against one piece of that
//! copper - a pad, a via, another net's trace. `a_contact_is_one_row` in
//! `cypcb-cli` holds the count to the same number however the copper is cut
//! into entities; these hold what the number means, on boards small enough to
//! work out by hand.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{ClearanceRule, DrcRule};
use cypcb_drc::DrcViolation;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position, RefDes,
    Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A 0.1mm trace: half-width 0.05mm.
const WIDTH_MM: f64 = 0.1;

fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
    world
}

fn square_pad(number: &str, x_mm: f64) -> PadDef {
    PadDef {
        number: number.to_string(),
        shape: PadShape::Rect,
        position: Point::from_mm(x_mm, 0.0),
        size: (Nm::from_mm(0.5), Nm::from_mm(0.5)),
        drill: None,
        slot: None,
        layers: vec![Layer::TopCopper],
        mask_margin: None,
    }
}

/// A part at (10, 10) with the pads given, pad `n` on net `n`.
fn part(world: &mut BoardWorld, pads: Vec<PadDef>) {
    let mut library = FootprintLibrary::new();
    let mut nets = NetConnections::new();
    for pad in &pads {
        let net: u32 = pad.number.parse().expect("pads are numbered");
        nets.add(PinConnection::new(pad.number.clone(), NetId::new(net)));
    }
    let box_ = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.0), Nm::from_mm(1.0)));
    library.register(Footprint {
        name: "PART".to_string(),
        description: "pads in a row".to_string(),
        pads,
        bounds: box_,
        courtyard: box_,
        silk: Vec::new(),
    });
    world.spawn_component(
        RefDes::new("U1"),
        Value::new("part"),
        Position(Point::from_mm(10.0, 10.0)),
        Rotation::from_degrees(0.0),
        FootprintRef::new("PART"),
        nets,
    );
    world.set_footprints(library);
}

fn trace(world: &mut BoardWorld, points: &[(f64, f64)], net: u32) {
    let segments = points
        .windows(2)
        .map(|w| {
            TraceSegment::new(
                Point::from_mm(w[0].0, w[0].1),
                Point::from_mm(w[1].0, w[1].1),
            )
        })
        .collect();
    world.spawn_entity((
        Trace {
            segments,
            width: Nm::from_mm(WIDTH_MM),
            layer: Layer::TopCopper,
            net_id: NetId::new(net),
            locked: false,
            source: TraceSource::Autorouted,
        },
        NetId::new(net),
    ));
}

fn clearance(world: &mut BoardWorld) -> Vec<DrcViolation> {
    world.rebuild_spatial_index_with_traces(|_| {
        Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.0), Nm::from_mm(1.0)))
    });
    let rules = DesignRules {
        min_clearance: Nm::from_mm(0.127),
        ..DesignRules::jlcpcb_2layer()
    };
    ClearanceRule.check(world, &rules)
}

fn places(found: &[DrcViolation]) -> Vec<(f64, f64, f64)> {
    found
        .iter()
        .map(|v| {
            (
                v.location.x.to_mm(),
                v.location.y.to_mm(),
                v.actual.map_or(-1.0, |a| a.to_mm()),
            )
        })
        .collect()
}

#[test]
fn a_run_past_two_pads_on_two_nets_is_two_places() {
    // Pads 0.5mm square with centres 0.8mm apart: pad 1 spans x 9.35..9.85,
    // pad 2 x 10.15..10.65, both y 9.75..10.25. The trace runs along y=10.35,
    // its copper 0.05mm off both pads' top edge. Where it is too close to pad 1
    // and where it is too close to pad 2 run into each other over the 0.3mm
    // between them - against the part as a whole it is one run - and they are
    // two gaps to two nets, so two rows.
    let mut world = board();
    part(
        &mut world,
        vec![square_pad("1", -0.4), square_pad("2", 0.4)],
    );
    trace(&mut world, &[(8.5, 10.35), (11.5, 10.35)], 3);

    let found = clearance(&mut world);
    assert_eq!(found.len(), 2, "one place per pad: {:?}", places(&found));
}

#[test]
fn a_run_along_one_pad_is_one_place_however_many_segments_it_has() {
    // The same 0.05mm gap along one pad's top edge, drawn as three segments on
    // one straight line. The copper is one run and the gap one gap.
    let mut world = board();
    part(&mut world, vec![square_pad("1", 0.0)]);
    trace(
        &mut world,
        &[(9.0, 10.35), (9.8, 10.35), (10.1, 10.35), (11.0, 10.35)],
        3,
    );

    let found = clearance(&mut world);
    assert_eq!(found.len(), 1, "one run, one place: {:?}", places(&found));
}

#[test]
fn a_trace_that_crosses_another_net_twice_is_two_places() {
    // Net 1 runs along y=10 from x=8 to x=12. Net 2 comes up through it at
    // x=9, runs across 1mm above it and comes back down through it at x=11:
    // two crossings 2mm apart, with no copper too close between them.
    let mut world = board();
    trace(&mut world, &[(8.0, 10.0), (12.0, 10.0)], 1);
    trace(
        &mut world,
        &[(9.0, 9.0), (9.0, 11.0), (11.0, 11.0), (11.0, 9.0)],
        2,
    );

    let found = clearance(&mut world);
    assert_eq!(found.len(), 2, "two crossings: {:?}", places(&found));
    assert!(
        found.iter().all(|v| v.actual == Some(Nm::ZERO)),
        "both cross: {:?}",
        places(&found)
    );
}
