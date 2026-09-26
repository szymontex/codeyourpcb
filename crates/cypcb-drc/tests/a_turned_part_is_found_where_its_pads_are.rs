//! A part turned on the board is found where its copper is.
//!
//! `cargo test -p cypcb-drc --test a_turned_part_is_found_where_its_pads_are`
//!
//! A component sits in the spatial index as its courtyard. The courtyard has
//! to turn with the part, or a part turned 90 degrees stands in the index
//! across the board from its own pads, and a query at a pad finds nothing.
//!
//! The part here is a bar: two pads 6mm apart on a courtyard 7mm by 1mm.
//! Turned 90 or 45 degrees, neither pad is inside the unturned box, so every
//! reader of the index is asked about copper the unturned box does not cover.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{
    ClearanceRule, DrcRule, EdgeClearanceRule, MountingHoleClearanceRule, SlotClearanceRule,
};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{
    place_pad, FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position,
    RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, Entity};

/// The angles every test turns the bar through. Zero is the control.
const ANGLES: [f64; 3] = [0.0, 90.0, 45.0];

/// Half the pad's side: the bar's pads are 0.6mm squares.
const HALF_PAD_MM: f64 = 0.3;

fn pad(number: &str, x_mm: f64) -> PadDef {
    PadDef {
        number: number.to_string(),
        shape: PadShape::Rect,
        position: Point::from_mm(x_mm, 0.0),
        size: (Nm::from_mm(0.6), Nm::from_mm(0.6)),
        drill: None,
        slot: None,
        layers: vec![Layer::TopCopper],
        mask_margin: None,
        rotation: Rotation::ZERO,
    }
}

fn bar() -> Footprint {
    let courtyard = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(7.0), Nm::from_mm(1.0)));
    Footprint {
        name: "BAR".to_string(),
        description: "two pads 6mm apart".to_string(),
        pads: vec![pad("1", -3.0), pad("2", 3.0)],
        bounds: courtyard,
        courtyard,
        silk: Vec::new(),
    }
}

/// The bar with a courtyard around pad 1 alone, the way a footprint centred on
/// its first pin states one: pad 2 stands outside the box the index holds.
fn short_bar() -> Footprint {
    Footprint {
        name: "SHORT-BAR".to_string(),
        courtyard: Rect::from_center_size(
            Point::from_mm(-3.0, 0.0),
            (Nm::from_mm(1.0), Nm::from_mm(1.0)),
        ),
        ..bar()
    }
}

/// An M3 mounting hole: 3.2mm drilled, no copper.
fn mounting_hole() -> Footprint {
    let drill = Nm::from_mm(3.2);
    Footprint {
        name: "MOUNT-M3".to_string(),
        description: "M3 mounting hole".to_string(),
        pads: vec![PadDef {
            number: String::new(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (drill, drill),
            drill: Some(drill),
            slot: None,
            layers: Vec::new(),
            mask_margin: None,
            rotation: Rotation::ZERO,
        }],
        bounds: Rect::from_center_size(Point::ORIGIN, (drill, drill)),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(7.2), Nm::from_mm(7.2))),
        silk: Vec::new(),
    }
}

/// A plated slot 2.4mm by 1.0mm along x, in a 3.2mm by 1.8mm pad.
fn latch(library: &FootprintLibrary) -> Footprint {
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    Footprint {
        name: "latch".to_string(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Oblong,
            position: Point::ORIGIN,
            size: (Nm::from_mm(3.2), Nm::from_mm(1.8)),
            drill: Some(Nm::from_mm(1.0)),
            slot: Some((Nm::from_mm(2.4), Nm::from_mm(1.0))),
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        }],
        ..base
    }
}

fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(bar());
    library.register(short_bar());
    library.register(mounting_hole());
    let latch = latch(&library);
    library.register(latch);
    world.set_footprints(library);
    world
}

/// The direction the bar points once turned: from pad 1 to pad 2.
fn along(degrees: f64) -> (f64, f64) {
    let r = degrees.to_radians();
    (r.cos(), r.sin())
}

/// Put the bar down turned by `degrees`, with pad 2 centred on `(x, y)`.
fn bar_with_pad_2_at(world: &mut BoardWorld, degrees: f64, x: f64, y: f64) -> Entity {
    footprint_with_pad_2_at(world, "BAR", degrees, x, y)
}

/// Put down a two-pad footprint laid out like the bar.
fn footprint_with_pad_2_at(
    world: &mut BoardWorld,
    footprint: &str,
    degrees: f64,
    x: f64,
    y: f64,
) -> Entity {
    let (ux, uy) = along(degrees);
    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1".to_string(), NetId::new(1)));
    nets.add(PinConnection::new("2".to_string(), NetId::new(2)));
    world.spawn_component(
        RefDes::new("U1"),
        Value::new("bar"),
        Position::from_mm(x - 3.0 * ux, y - 3.0 * uy),
        Rotation::from_degrees(degrees),
        FootprintRef::new(footprint),
        nets,
    )
}

fn part(world: &mut BoardWorld, refdes: &str, footprint: &str, x: f64, y: f64) {
    world.spawn_component(
        RefDes::new(refdes),
        Value::new(""),
        Position::from_mm(x, y),
        Rotation::ZERO,
        FootprintRef::new(footprint),
        NetConnections::new(),
    );
}

fn index(world: &mut BoardWorld) {
    let library = world.footprints().clone();
    world.rebuild_spatial_index_from_library(&library);
}

fn rules() -> DesignRules {
    DesignRules::jlcpcb_2layer()
}

/// Asks `found` at every angle and holds each answer to one row.
fn one_row_at_every_angle(found: impl Fn(f64) -> usize) {
    let counts: Vec<(f64, usize)> = ANGLES.iter().map(|&d| (d, found(d))).collect();
    assert!(
        counts.iter().all(|&(_, n)| n == 1),
        "rows found (degrees, rows), one expected at each: {counts:?}"
    );
}

#[test]
fn the_index_finds_the_part_at_each_of_its_pads() {
    let mut missed = Vec::new();
    for degrees in ANGLES {
        let mut world = board();
        let bar = bar_with_pad_2_at(&mut world, degrees, 10.0, 10.0);
        index(&mut world);
        let centre = world.get::<Position>(bar).expect("placed").0;
        for x in [-3.0, 3.0] {
            let at = place_pad(
                centre,
                Point::from_mm(x, 0.0),
                Rotation::from_degrees(degrees),
            );
            if !world.query_point(at).contains(&bar) {
                missed.push((degrees, at.x.to_mm(), at.y.to_mm()));
            }
        }
    }
    assert!(
        missed.is_empty(),
        "the index has no part at these pads (degrees, x, y): {missed:?}"
    );
}

/// Clearance rows for a trace beside pad 2 of `footprint`, turned by `degrees`.
///
/// A 0.1mm trace on net 3 runs straight out from pad 2 along the bar, its
/// round end 0.45mm from the pad's centre: 0.45 - 0.05 - 0.3 = 0.10mm of gap
/// against 0.127mm asked.
fn clearance_rows_beside_pad_2(footprint: &str, degrees: f64) -> usize {
    let mut world = board();
    footprint_with_pad_2_at(&mut world, footprint, degrees, 10.0, 10.0);
    let (ux, uy) = along(degrees);
    let from = Point::from_mm(10.0 + 0.45 * ux, 10.0 + 0.45 * uy);
    let to = Point::from_mm(10.0 + 2.0 * ux, 10.0 + 2.0 * uy);
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(from, to)],
            width: Nm::from_mm(0.1),
            layer: Layer::TopCopper,
            net_id: NetId::new(3),
            locked: false,
            source: TraceSource::Autorouted,
        },
        NetId::new(3),
    ));
    index(&mut world);
    let rules = DesignRules {
        min_clearance: Nm::from_mm(0.127),
        ..rules()
    };
    ClearanceRule.check(&mut world, &rules).len()
}

#[test]
fn clearance_finds_a_trace_beside_a_turned_pad() {
    // The trace's box is nowhere near the unturned courtyard once the bar is
    // turned.
    one_row_at_every_angle(|degrees| clearance_rows_beside_pad_2("BAR", degrees));
}

#[test]
fn clearance_finds_a_trace_beside_a_pad_outside_the_courtyard() {
    // The index holds only the box around pad 1, so a query from the trace
    // finds no part. The pair is found from the part's side, which looks as
    // far as its pads reach.
    one_row_at_every_angle(|degrees| clearance_rows_beside_pad_2("SHORT-BAR", degrees));
}

#[test]
fn edge_clearance_finds_a_turned_pad_at_the_edge() {
    // Pad 2's centre 0.5mm inside the top edge: its side is 0.2mm from the
    // edge square on, its corner 0.08mm turned 45 degrees.
    one_row_at_every_angle(|degrees| {
        let mut world = board();
        bar_with_pad_2_at(&mut world, degrees, 10.0, 19.5);
        index(&mut world);
        EdgeClearanceRule.check(&mut world, &rules()).len()
    });
}

#[test]
fn mounting_hole_clearance_finds_a_turned_pad_beside_the_hole() {
    // Pad 2's centre 2.0mm from the centre of a 3.2mm hole, along the bar:
    // 2.0 - 1.6 - 0.3 = 0.1mm of laminate.
    one_row_at_every_angle(|degrees| {
        let mut world = board();
        part(&mut world, "H1", "MOUNT-M3", 10.0, 10.0);
        let (ux, uy) = along(degrees);
        let reach = 1.6 + HALF_PAD_MM + 0.1;
        bar_with_pad_2_at(&mut world, degrees, 10.0 + reach * ux, 10.0 + reach * uy);
        index(&mut world);
        MountingHoleClearanceRule.check(&mut world, &rules()).len()
    });
}

#[test]
fn slot_clearance_finds_a_turned_pad_beside_the_slot() {
    // The slot's wall is 0.5mm above its centre. Pad 2 sits over it: square
    // on, its lower side 0.1mm from the wall; turned 45 degrees, its lower
    // corner 0.176mm from it. Both are under 0.3mm.
    one_row_at_every_angle(|degrees| {
        let mut world = board();
        part(&mut world, "J1", "latch", 10.0, 10.0);
        let y = if degrees == 45.0 { 11.1 } else { 10.9 };
        bar_with_pad_2_at(&mut world, degrees, 10.0, y);
        index(&mut world);
        SlotClearanceRule.check(&mut world, &rules()).len()
    });
}
