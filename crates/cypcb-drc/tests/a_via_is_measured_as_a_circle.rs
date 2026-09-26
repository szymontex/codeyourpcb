//! A via is a disc of copper, and a circular pad is one too.
//!
//! `cargo test -p cypcb-drc --test a_via_is_measured_as_a_circle`
//!
//! `ClearanceRule` measured both as the square around the disc. A trace that
//! passes the corner of that square clears the copper by the corner's cut,
//! `r * (sqrt(2) - 1)`, which on a 0.6mm via is 0.124mm, and read as touching
//! it: on `multi_ic` a gap of 0.033mm came out as 0.00mm, a short, and two vias
//! whose discs were 0.166mm apart were reported as touching. `NetSplitRule`
//! asks the same question at zero, so the same corner joined copper that does
//! not meet.
//!
//! Every case below sits on the diagonal, where the square and the disc
//! disagree the most, and every distance is worked from the centre.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{ClearanceRule, DrcRule, NetSplitRule};
use cypcb_drc::{shorts, ViolationKind};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position, RefDes,
    Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A 0.6mm via, the default: radius 0.3mm.
const VIA_RADIUS_NM: i64 = 300_000;
/// A 0.1mm trace: half-width 0.05mm.
const HALF_WIDTH_NM: i64 = 50_000;

fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
    world
}

fn via(world: &mut BoardWorld, x_nm: i64, y_nm: i64, net: NetId) {
    world.spawn_entity((Via::new(Point::new(Nm(x_nm), Nm(y_nm)), net), net));
}

fn trace(world: &mut BoardWorld, from: (i64, i64), to: (i64, i64), net: NetId) {
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                Point::new(Nm(from.0), Nm(from.1)),
                Point::new(Nm(to.0), Nm(to.1)),
            )],
            width: Nm(2 * HALF_WIDTH_NM),
            layer: Layer::TopCopper,
            net_id: net,
            locked: false,
            source: TraceSource::Autorouted,
        },
        net,
    ));
}

/// A trace on the line `x + y = 2 * centre + offset`, crossing the diagonal
/// through the centre: its centreline is `offset / sqrt(2)` from the centre.
fn diagonal_trace(world: &mut BoardWorld, centre: i64, centreline_nm: f64, net: NetId) {
    let offset = (centreline_nm * std::f64::consts::SQRT_2).round() as i64;
    let sum = 2 * centre + offset;
    let (x1, x2) = (centre - 2_000_000, centre + 2_000_000);
    trace(world, (x1, sum - x1), (x2, sum - x2), net);
}

fn clearance(world: &mut BoardWorld, min_clearance_mm: f64) -> Vec<cypcb_drc::DrcViolation> {
    world.rebuild_spatial_index_with_traces(|_| {
        Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0)))
    });
    let rules = DesignRules {
        min_clearance: Nm::from_mm(min_clearance_mm),
        ..DesignRules::jlcpcb_2layer()
    };
    ClearanceRule.check(world, &rules)
}

fn actual_mm(violations: &[cypcb_drc::DrcViolation]) -> Vec<f64> {
    violations
        .iter()
        .map(|v| v.actual.map(|n| n.0 as f64 / 1e6).unwrap_or(f64::NAN))
        .collect()
}

#[test]
fn a_trace_past_the_corner_of_a_vias_box_is_a_gap_and_not_a_short() {
    let mut world = board();
    let centre = 10_000_000;
    via(&mut world, centre, centre, NetId::new(1));
    // Copper to copper: 0.383 - 0.300 - 0.050 = 0.033mm. The centreline cuts
    // the corner of the via's square, which is 0.424mm out on the diagonal.
    diagonal_trace(&mut world, centre, 383_000.0, NetId::new(2));

    let found = clearance(&mut world, 0.127);

    assert_eq!(
        found.len(),
        1,
        "one gap under 0.127mm: {:?}",
        actual_mm(&found)
    );
    assert_eq!(found[0].kind, ViolationKind::Clearance);
    assert_eq!(
        shorts(&found),
        0,
        "0.033mm of board between the copper is not a short"
    );
    let gap = found[0].actual.unwrap().0;
    assert!(
        (32_000..=34_000).contains(&gap),
        "the gap is 0.033mm from the disc, measured {}mm",
        gap as f64 / 1e6
    );
}

#[test]
fn two_vias_whose_discs_are_apart_are_not_too_close() {
    let mut world = board();
    // Centres 0.766mm apart on the diagonal: 0.766 - 2 * 0.3 = 0.166mm of
    // board between the discs, while their squares overlap by 0.058mm.
    let step = (766_000.0 / std::f64::consts::SQRT_2).round() as i64;
    via(&mut world, 10_000_000, 10_000_000, NetId::new(1));
    via(
        &mut world,
        10_000_000 + step,
        10_000_000 + step,
        NetId::new(2),
    );

    let found = clearance(&mut world, 0.10);

    assert!(
        found.is_empty(),
        "0.166mm between the discs clears 0.10mm, the checker measured {:?}",
        actual_mm(&found)
    );
}

#[test]
fn two_vias_whose_discs_overlap_are_still_a_short() {
    let mut world = board();
    // Centres 0.5mm apart on the diagonal: the discs overlap by 0.1mm.
    let step = (500_000.0 / std::f64::consts::SQRT_2).round() as i64;
    via(&mut world, 10_000_000, 10_000_000, NetId::new(1));
    via(
        &mut world,
        10_000_000 + step,
        10_000_000 + step,
        NetId::new(2),
    );

    let found = clearance(&mut world, 0.10);

    assert_eq!(
        shorts(&found),
        1,
        "discs that overlap are copper touching copper: {:?}",
        actual_mm(&found)
    );
}

#[test]
fn a_trace_across_a_via_is_still_a_short() {
    let mut world = board();
    let centre = 10_000_000;
    via(&mut world, centre, centre, NetId::new(1));
    // Centreline 0.3mm from the centre: the trace's copper covers the disc's edge.
    diagonal_trace(&mut world, centre, 300_000.0, NetId::new(2));

    let found = clearance(&mut world, 0.127);

    assert_eq!(
        shorts(&found),
        1,
        "a trace over a via's copper is a short: {:?}",
        actual_mm(&found)
    );
}

/// One circular pad of 0.6mm at the origin of the part, on net 1.
fn round_pad_part(world: &mut BoardWorld, at_nm: i64) {
    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "ROUND".to_string(),
        description: "one circular pad".to_string(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (Nm(2 * VIA_RADIUS_NM), Nm(2 * VIA_RADIUS_NM)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        }],
        bounds: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(0.6), Nm::from_mm(0.6))),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0))),
        silk: Vec::new(),
    });
    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1".to_string(), NetId::new(1)));
    world.spawn_component(
        RefDes::new("TP1"),
        Value::new("pad"),
        Position(Point::new(Nm(at_nm), Nm(at_nm))),
        Rotation(0),
        FootprintRef::new("ROUND"),
        nets,
    );
    world.set_footprints(library);
}

#[test]
fn a_trace_past_the_corner_of_a_round_pads_box_is_a_gap_and_not_a_short() {
    let mut world = board();
    let centre = 10_000_000;
    round_pad_part(&mut world, centre);
    diagonal_trace(&mut world, centre, 383_000.0, NetId::new(2));

    let found = clearance(&mut world, 0.127);

    assert_eq!(
        found.len(),
        1,
        "one gap under 0.127mm: {:?}",
        actual_mm(&found)
    );
    assert_eq!(shorts(&found), 0, "0.033mm from a round pad is not a short");
    let gap = found[0].actual.unwrap().0;
    assert!(
        (32_000..=34_000).contains(&gap),
        "the gap is 0.033mm from the pad's disc, measured {}mm",
        gap as f64 / 1e6
    );
}

/// Two single-pad parts on net 1, pads at (4.5, 10) and (15.5, 10); a trace
/// from the first stops `end_nm` from a via at (10, 10) on the diagonal, and a
/// trace from the via runs on to the second.
fn net_through_a_via(end_nm: f64) -> Vec<cypcb_drc::DrcViolation> {
    let mut world = board();
    let net = NetId::new(1);
    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "PAD".to_string(),
        description: "one square pad".to_string(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(0.6), Nm::from_mm(0.6)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        }],
        bounds: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(0.6), Nm::from_mm(0.6))),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0))),
        silk: Vec::new(),
    });
    for (refdes, x) in [("J1", 4.5), ("J2", 15.5)] {
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1".to_string(), net));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("pad"),
            Position::from_mm(x, 10.0),
            Rotation(0),
            FootprintRef::new("PAD"),
            nets,
        );
    }
    world.set_footprints(library);

    let centre = 10_000_000;
    let back = (end_nm / std::f64::consts::SQRT_2).round() as i64;
    via(&mut world, centre, centre, net);
    trace(
        &mut world,
        (4_500_000, centre),
        (centre - back, centre - back),
        net,
    );
    trace(&mut world, (centre, centre), (15_500_000, centre), net);
    world.rebuild_spatial_index_with_traces(|_| {
        Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0)))
    });

    NetSplitRule.check(&mut world, &DesignRules::jlcpcb_2layer())
}

#[test]
fn a_trace_ending_in_the_corner_of_a_vias_box_does_not_join_the_via() {
    // The end is 0.37mm from the centre: 0.02mm of board between the trace's
    // copper and the disc, and inside the via's square.
    let found = net_through_a_via(370_000.0);

    assert_eq!(
        found.len(),
        1,
        "copper that does not meet leaves the net in two pieces: {:?}",
        found.iter().map(|v| v.message.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn a_trace_ending_on_a_vias_disc_joins_it() {
    // The end is 0.34mm from the centre: the trace's copper overlaps the disc
    // by 0.01mm.
    let found = net_through_a_via(340_000.0);

    assert!(
        found.is_empty(),
        "copper that meets is one piece: {:?}",
        found.iter().map(|v| v.message.clone()).collect::<Vec<_>>()
    );
}
