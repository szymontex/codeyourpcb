//! Clearance is a copper rule, so it has to be measured against copper.
//!
//! A component's entry in the spatial index is its **courtyard** - the
//! assembly keepout that covers the part body. Measuring a trace against that
//! box calls a trace running through the gap between two pads a short, which
//! is normal manufacturing and which every real board does. Component bodies
//! are covered by `CourtyardClearanceRule`; this rule must look at pads.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{ClearanceRule, CourtyardClearanceRule, DrcRule};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::PadShape;
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PinConnection, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A 0402-shaped part: two pads 1mm apart, a courtyard covering both and the
/// body between them.
///
/// Pad copper spans x -0.8..-0.2 and 0.2..0.8, so the gap between the pads is
/// 0.4mm wide and the courtyard is 2.0 x 1.2mm.
fn two_pad_footprint() -> Footprint {
    let pad = |number: &str, x: f64| PadDef {
        number: number.to_string(),
        shape: PadShape::Rect,
        position: Point::from_mm(x, 0.0),
        size: (Nm::from_mm(0.6), Nm::from_mm(0.6)),
        drill: None,
        layers: vec![Layer::TopCopper],
    };

    Footprint {
        name: "R_0402".to_string(),
        description: "two pads and a body".to_string(),
        pads: vec![pad("1", -0.5), pad("2", 0.5)],
        bounds: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(0.5))),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.0), Nm::from_mm(1.2))),
        silk: Vec::new(),
    }
}

/// A board with one two-pad part at 10mm, 10mm on nets 1 and 2.
fn board_with_part() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

    let mut library = FootprintLibrary::new();
    library.register(two_pad_footprint());

    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1".to_string(), NetId::new(1)));
    nets.add(PinConnection::new("2".to_string(), NetId::new(2)));

    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(10.0, 10.0),
        Rotation(0),
        FootprintRef::new("R_0402"),
        nets,
    );

    world.set_footprints(library.clone());
    (world, library)
}

/// Spawn a foreign-net trace running vertically at `x`, on the top layer.
fn spawn_vertical_trace(world: &mut BoardWorld, x: f64) {
    let net = NetId::new(99);
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(x, 8.0),
                Point::from_mm(x, 12.0),
            )],
            width: Nm::from_mm(0.1),
            layer: Layer::TopCopper,
            net_id: net,
            locked: false,
            source: TraceSource::Autorouted,
        },
        net,
    ));
}

#[test]
fn a_trace_through_the_gap_between_two_pads_is_not_a_clearance_violation() {
    let (mut world, library) = board_with_part();

    // Dead centre between the pads: the nearest pad copper is 0.2mm away, and
    // the trace's own half-width is 0.05mm, so the copper gap is 0.15mm -
    // above the 0.127mm JLCPCB minimum. Inside the courtyard the whole way.
    spawn_vertical_trace(&mut world, 10.0);
    world.rebuild_spatial_index_from_library(&library);

    let violations = ClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());

    assert!(
        violations.is_empty(),
        "a trace in the gap between two pads clears both by 0.15mm, \
         but the checker reported: {:?}",
        violations
            .iter()
            .map(|v| v.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_trace_across_a_pad_is_still_a_clearance_violation() {
    let (mut world, library) = board_with_part();

    // Straight over pad 2's copper, on a net the part does not carry.
    spawn_vertical_trace(&mut world, 10.5);
    world.rebuild_spatial_index_from_library(&library);

    let violations = ClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());

    assert_eq!(
        violations.len(),
        1,
        "a trace crossing a foreign pad is a short and has to be reported"
    );
}

#[test]
fn a_trace_below_a_top_only_pad_is_on_its_own_layer() {
    let (mut world, library) = board_with_part();

    // Same overlap as above, but on the bottom layer, where this footprint has
    // no copper at all.
    let net = NetId::new(99);
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(10.5, 8.0),
                Point::from_mm(10.5, 12.0),
            )],
            width: Nm::from_mm(0.1),
            layer: Layer::BottomCopper,
            net_id: net,
            locked: false,
            source: TraceSource::Autorouted,
        },
        net,
    ));
    world.rebuild_spatial_index_from_library(&library);

    let violations = ClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());

    assert!(
        violations.is_empty(),
        "a bottom-layer trace cannot short a pad that only exists on top: {:?}",
        violations
            .iter()
            .map(|v| v.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn overlapping_part_bodies_are_still_reported_by_the_courtyard_rule() {
    // Refining the clearance rule to pads must not make placement collisions
    // disappear - that is the courtyard rule's job, and it has to actually do
    // it.
    let (mut world, library) = board_with_part();

    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1".to_string(), NetId::new(3)));
    world.spawn_component(
        RefDes::new("R2"),
        Value::new("10k"),
        Position::from_mm(10.5, 10.0), // courtyards are 2mm wide: a 1.5mm overlap
        Rotation(0),
        FootprintRef::new("R_0402"),
        nets,
    );
    world.rebuild_spatial_index_from_library(&library);

    let violations = CourtyardClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());

    assert_eq!(
        violations.len(),
        1,
        "two parts sitting on top of each other is a placement error and has \
         to be reported: {:?}",
        violations
            .iter()
            .map(|v| v.message.clone())
            .collect::<Vec<_>>()
    );
}
