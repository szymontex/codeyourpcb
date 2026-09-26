//! Copper beside the board edge, a slot or a bare hole is measured as copper.
//!
//! `cargo test -p cypcb-drc --test copper_near_an_opening_is_measured_as_copper`
//!
//! A trace segment sits in the spatial index as the box around it, grown by
//! half its width, and a via as the square around its disc. The edge, slot and
//! mounting-hole rules measured those boxes, so a diagonal trace read as close
//! to an opening as the corner of its box, and a via as the corner of its
//! square - copper that is not there. `ClearanceRule` measures a trace by its
//! centreline and a via by its disc; these three now do the same.
//!
//! Every case below is built the same way: copper whose real gap is the
//! required clearance plus a margin, placed where its box reaches inside the
//! clearance. Each test checks three things: the copper passes; the same box,
//! standing in the index for nothing but itself, is reported, so the geometry
//! really does tell the two apart; and the copper moved in by twice the margin
//! is reported, so the rule still sees a real fault.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{DrcRule, EdgeClearanceRule, MountingHoleClearanceRule, SlotClearanceRule};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::components::{
    BoardOutline, FootprintRef, Layer, NetConnections, NetId, PadShape, Position, RefDes, Rotation,
    Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, Entity, SpatialEntry, SpatialIndex};

/// How far past the required clearance the copper sits, in millimetres.
const MARGIN: f64 = 0.05;
/// The trace width every case uses, in millimetres.
const WIDTH: f64 = 0.2;
/// The via's copper and drill diameters, in millimetres.
const VIA_OUTER: f64 = 0.6;
const VIA_DRILL: f64 = 0.3;

const DIAGONAL: f64 = std::f64::consts::FRAC_1_SQRT_2;

fn at(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn net() -> NetId {
    NetId::new(7)
}

/// A trace segment centred on `(x, y)`, running along `(1, -1)` for `half`
/// millimetres either side.
fn spawn_diagonal_trace(world: &mut BoardWorld, (x, y): (f64, f64), half: f64) -> Entity {
    let run = half * DIAGONAL;
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                at(x - run, y + run),
                at(x + run, y - run),
            )],
            width: Nm::from_mm(WIDTH),
            layer: Layer::TopCopper,
            net_id: net(),
            locked: false,
            source: TraceSource::Autorouted,
        },
        net(),
    ))
}

fn spawn_via(world: &mut BoardWorld, (x, y): (f64, f64)) -> Entity {
    world.spawn_entity((
        Via {
            position: at(x, y),
            drill: Nm::from_mm(VIA_DRILL),
            outer_diameter: Nm::from_mm(VIA_OUTER),
            net_id: net(),
            start_layer: Layer::TopCopper,
            end_layer: Layer::BottomCopper,
            locked: false,
        },
        net(),
    ))
}

/// A point `distance` millimetres from `from`, along `(1, 1)`.
fn out_along_the_diagonal((x, y): (f64, f64), distance: f64) -> (f64, f64) {
    (x + distance * DIAGONAL, y + distance * DIAGONAL)
}

/// Index the board, then add a copy of every entry `copper` has under a bare
/// entity - the box, standing for nothing but itself.
fn index_with_the_box_alone(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    copper: Entity,
) -> Entity {
    world.rebuild_spatial_index_from_library(library);
    let bare = world.ecs_mut().spawn(()).id();
    let mut entries: Vec<SpatialEntry> = world.spatial().iter().cloned().collect();
    let boxes: Vec<SpatialEntry> = entries
        .iter()
        .filter(|entry| entry.entity == copper)
        .map(|entry| SpatialEntry {
            entity: bare,
            ..entry.clone()
        })
        .collect();
    assert!(!boxes.is_empty(), "the copper is in the index");
    entries.extend(boxes);
    world
        .ecs_mut()
        .resource_mut::<SpatialIndex>()
        .rebuild(entries);
    bare
}

/// The three checks every case makes. `build` places the copper `gap`
/// millimetres from the opening and returns the board and the copper.
fn copper_is_measured_not_its_box(
    rule: &dyn DrcRule,
    required: f64,
    build: impl Fn(f64) -> (BoardWorld, FootprintLibrary, Entity),
) {
    let rules = DesignRules::jlcpcb_2layer();

    let (mut world, library, copper) = build(required + MARGIN);
    let bare = index_with_the_box_alone(&mut world, &library, copper);
    let reported: Vec<Entity> = rule
        .check(&mut world, &rules)
        .iter()
        .map(|v| v.entity)
        .collect();
    assert!(
        !reported.contains(&copper),
        "{}: copper {MARGIN}mm past the required {required}mm is reported: {reported:?}",
        rule.name()
    );
    assert!(
        reported.contains(&bare),
        "{}: the box around the same copper is not reported, so this case does not \
         tell copper from its box: {reported:?}",
        rule.name()
    );

    let (mut world, library, copper) = build(required - MARGIN);
    world.rebuild_spatial_index_from_library(&library);
    let reported: Vec<Entity> = rule
        .check(&mut world, &rules)
        .iter()
        .map(|v| v.entity)
        .collect();
    assert!(
        reported.contains(&copper),
        "{}: copper {MARGIN}mm inside the required {required}mm is not reported: {reported:?}",
        rule.name()
    );
}

// ---------------------------------------------------------------- the edge

/// A 20 x 20mm board with its corner at the origin cut off by a 45-degree
/// chamfer along `x + y = 5`.
fn chamfered_board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "chamfer".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    let board = world.board_entity().expect("a board");
    let outline = BoardOutline::new(vec![
        at(5.0, 0.0),
        at(20.0, 0.0),
        at(20.0, 20.0),
        at(0.0, 20.0),
        at(0.0, 5.0),
    ])
    .expect("a ring");
    world.ecs_mut().entity_mut(board).insert(outline);
    let library = FootprintLibrary::new();
    world.set_footprints(library.clone());
    (world, library)
}

/// The middle of the chamfer.
const CHAMFER_MIDDLE: (f64, f64) = (2.5, 2.5);

#[test]
fn a_diagonal_trace_beside_a_chamfer_is_measured_by_its_centreline() {
    let required = DesignRules::jlcpcb_2layer().min_edge_clearance.to_mm();
    copper_is_measured_not_its_box(&EdgeClearanceRule, required, |gap| {
        let (mut world, library) = chamfered_board();
        let centre = out_along_the_diagonal(CHAMFER_MIDDLE, gap + WIDTH / 2.0);
        let trace = spawn_diagonal_trace(&mut world, centre, 1.5);
        (world, library, trace)
    });
}

#[test]
fn a_via_beside_a_chamfer_is_measured_as_a_disc() {
    let required = DesignRules::jlcpcb_2layer().min_edge_clearance.to_mm();
    copper_is_measured_not_its_box(&EdgeClearanceRule, required, |gap| {
        let (mut world, library) = chamfered_board();
        let centre = out_along_the_diagonal(CHAMFER_MIDDLE, gap + VIA_OUTER / 2.0);
        let via = spawn_via(&mut world, centre);
        (world, library, via)
    });
}

// ---------------------------------------------------------------- a slot

/// Where the slot's right-hand bit centre sits: a 2.4 x 1.0mm slot on a part
/// at (10, 10) has its axis from 9.3 to 10.7mm and a 0.5mm radius.
const SLOT_END: (f64, f64) = (10.7, 10.0);
const SLOT_RADIUS: f64 = 0.5;

fn board_with_a_slot() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "slotted".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(Footprint {
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
    });
    world.set_footprints(library.clone());
    world.spawn_component(
        RefDes::new("J1"),
        Value::new(""),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("latch"),
        NetConnections::new(),
    );
    (world, library)
}

#[test]
fn a_diagonal_trace_past_a_slot_end_is_measured_by_its_centreline() {
    let required = DesignRules::jlcpcb_2layer().min_slot_clearance.to_mm();
    copper_is_measured_not_its_box(&SlotClearanceRule, required, |gap| {
        let (mut world, library) = board_with_a_slot();
        let centre = out_along_the_diagonal(SLOT_END, SLOT_RADIUS + gap + WIDTH / 2.0);
        let trace = spawn_diagonal_trace(&mut world, centre, 3.0);
        (world, library, trace)
    });
}

#[test]
fn a_via_past_a_slot_end_is_measured_as_a_disc() {
    let required = DesignRules::jlcpcb_2layer().min_slot_clearance.to_mm();
    copper_is_measured_not_its_box(&SlotClearanceRule, required, |gap| {
        let (mut world, library) = board_with_a_slot();
        let centre = out_along_the_diagonal(SLOT_END, SLOT_RADIUS + gap + VIA_OUTER / 2.0);
        let via = spawn_via(&mut world, centre);
        (world, library, via)
    });
}

// ---------------------------------------------------------- a mounting hole

const HOLE_CENTRE: (f64, f64) = (10.0, 10.0);
const HOLE_DRILL: f64 = 3.2;

/// A 20 x 20mm board with an unplated M3 hole at its centre.
fn board_with_a_hole() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "hole".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    let drill = Nm::from_mm(HOLE_DRILL);
    let mut library = FootprintLibrary::new();
    library.register(Footprint {
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
    });
    world.set_footprints(library.clone());
    world.spawn_component(
        RefDes::new("H1"),
        Value::new("M3"),
        Position::from_mm(HOLE_CENTRE.0, HOLE_CENTRE.1),
        Rotation(0),
        FootprintRef::new("MOUNT-M3"),
        NetConnections::new(),
    );
    (world, library)
}

#[test]
fn a_diagonal_trace_past_a_mounting_hole_is_measured_by_its_centreline() {
    let required = DesignRules::jlcpcb_2layer().min_edge_clearance.to_mm();
    copper_is_measured_not_its_box(&MountingHoleClearanceRule, required, |gap| {
        let (mut world, library) = board_with_a_hole();
        let centre = out_along_the_diagonal(HOLE_CENTRE, HOLE_DRILL / 2.0 + gap + WIDTH / 2.0);
        let trace = spawn_diagonal_trace(&mut world, centre, 2.0);
        (world, library, trace)
    });
}

#[test]
fn a_via_past_a_mounting_hole_is_measured_as_a_disc() {
    let required = DesignRules::jlcpcb_2layer().min_edge_clearance.to_mm();
    copper_is_measured_not_its_box(&MountingHoleClearanceRule, required, |gap| {
        let (mut world, library) = board_with_a_hole();
        let centre = out_along_the_diagonal(HOLE_CENTRE, HOLE_DRILL / 2.0 + gap + VIA_OUTER / 2.0);
        let via = spawn_via(&mut world, centre);
        (world, library, via)
    });
}
