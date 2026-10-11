//! The checker measures a hole where the pad puts it, not in the middle of the
//! copper.
//!
//! `cargo test -p cypcb-drc --test a_hole_off_the_middle_of_its_pad_is_measured_where_it_is`
//!
//! A pad can state its hole off the middle of its land
//! ([`PadDef::hole_offset`]): a TO-92 for hand soldering puts it 0.4mm off, and
//! EasyEDA and KiCad both write it. The annular ring is the narrowest copper
//! between the hole and the edge of the land, so it is narrower on the side the
//! hole moved towards, and two holes that moved towards each other are nearer
//! than their pads are. Both rules measured from the pad's middle.
//!
//! The land here is 2.0 by 3.0mm with a 0.8mm hole. Centred, the ring is 0.6mm
//! on the short side and 1.1mm on the long one; with the hole 0.55mm from the
//! middle along x it is 0.05mm, against a 0.15mm minimum. Every case has its
//! control: the same land with the hole in the middle, or moved the same
//! distance along the side that has the room.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::{run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

fn pad(
    number: &str,
    x: f64,
    shape: PadShape,
    size: (f64, f64),
    offset: Option<(f64, f64)>,
) -> PadDef {
    PadDef {
        number: number.to_string(),
        shape,
        position: Point::from_mm(x, 0.0),
        size: (Nm::from_mm(size.0), Nm::from_mm(size.1)),
        drill: Some(Nm::from_mm(0.8)),
        slot: None,
        layers: vec![Layer::TopCopper, Layer::BottomCopper],
        mask_margin: None,
        rotation: Rotation::ZERO,
        hole_offset: offset.map(|(x, y)| Point::from_mm(x, y)),
    }
}

/// One part at (10, 10) holding `pads`, the first turned `own` in its footprint.
fn board(mut pads: Vec<PadDef>, own: Rotation) -> BoardWorld {
    pads[0].rotation = own;
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(8.0), Nm::from_mm(6.0)));
    library.register_design(Footprint {
        name: "held".to_string(),
        description: String::new(),
        pads,
        bounds: body,
        courtyard: body,
        silk: Vec::new(),
    });
    world.set_footprints(library);
    world.spawn_component(
        RefDes::new("J1"),
        Value::new("v"),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("held"),
        NetConnections::new(),
    );
    world
}

/// Where each fault of `kind` is, in millimetres.
fn faults(world: &mut BoardWorld, kind: ViolationKind) -> Vec<(f64, f64)> {
    run_drc(world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| violation.kind == kind)
        .map(|violation| {
            (
                violation.location.x.0 as f64 / 1e6,
                violation.location.y.0 as f64 / 1e6,
            )
        })
        .collect()
}

fn rings(pads: Vec<PadDef>, own: Rotation) -> Vec<(f64, f64)> {
    faults(&mut board(pads, own), ViolationKind::AnnularRing)
}

fn at(found: &[(f64, f64)], want: (f64, f64)) -> bool {
    found.len() == 1 && (found[0].0 - want.0).abs() < 1e-6 && (found[0].1 - want.1).abs() < 1e-6
}

#[test]
fn the_ring_is_narrow_on_the_side_the_hole_moved_towards() {
    let narrow = rings(
        vec![pad("1", 0.0, PadShape::Rect, (2.0, 3.0), Some((0.55, 0.0)))],
        Rotation::ZERO,
    );
    // The fault is at the hole, 0.55mm to the right of the part's origin.
    assert!(at(&narrow, (10.55, 10.0)), "{narrow:?}");
}

#[test]
fn control_the_hole_in_the_middle_has_the_ring_a_centred_hole_has() {
    let none = rings(
        vec![pad("1", 0.0, PadShape::Rect, (2.0, 3.0), None)],
        Rotation::ZERO,
    );
    assert!(none.is_empty(), "{none:?}");
}

#[test]
fn control_the_same_distance_along_the_long_side_leaves_the_ring_wide() {
    // 0.55mm up a 3.0mm land: the long side has 1.5mm to the edge, the ring is
    // 1.5 - 0.55 - 0.4 = 0.55mm.
    let wide = rings(
        vec![pad("1", 0.0, PadShape::Rect, (2.0, 3.0), Some((0.0, 0.55)))],
        Rotation::ZERO,
    );
    assert!(wide.is_empty(), "{wide:?}");
}

#[test]
fn the_offset_turns_with_the_pad_in_its_footprint() {
    // The offset is stated in the pad's own frame. The pad turned 90 degrees
    // puts the same 0.55mm along the part's y - and the land, turned too, has
    // its 2.0mm side along y, so the ring is as narrow as before and the fault
    // is 0.55mm above the origin.
    let turned = rings(
        vec![pad("1", 0.0, PadShape::Rect, (2.0, 3.0), Some((0.55, 0.0)))],
        Rotation::DEG_90,
    );
    assert!(at(&turned, (10.0, 10.55)), "{turned:?}");
}

#[test]
fn a_rounded_land_has_less_copper_in_its_corner_than_a_square_one() {
    // 0.3mm right and 0.9mm up on a 2.0 by 3.0mm land. On a rectangle the
    // nearest edge is the top one, 0.6mm off: the ring is 0.2mm. On an oblong
    // the corner is a quarter circle 1.0mm in radius, whose centre is 0.5mm up
    // from the middle: the hole is 0.5mm from that centre, 0.5mm from the edge,
    // and the ring is 0.1mm.
    let square = rings(
        vec![pad("1", 0.0, PadShape::Rect, (2.0, 3.0), Some((0.3, 0.9)))],
        Rotation::ZERO,
    );
    assert!(square.is_empty(), "{square:?}");
    let round = rings(
        vec![pad(
            "1",
            0.0,
            PadShape::Oblong,
            (2.0, 3.0),
            Some((0.3, 0.9)),
        )],
        Rotation::ZERO,
    );
    assert!(at(&round, (10.3, 10.9)), "{round:?}");
}

/// Two parts 2.54mm apart, each holding one 4.0 by 3.0mm land whose hole is
/// `offset` from its middle; the second part turned `second`.
fn two_parts(offset: Option<(f64, f64)>, second: Rotation) -> BoardWorld {
    let mut world = board(
        vec![pad("1", 0.0, PadShape::Rect, (4.0, 3.0), offset)],
        Rotation::ZERO,
    );
    world.spawn_component(
        RefDes::new("J2"),
        Value::new("v"),
        Position::from_mm(12.54, 10.0),
        second,
        FootprintRef::new("held"),
        NetConnections::new(),
    );
    world
}

#[test]
fn two_holes_that_moved_towards_each_other_are_nearer_than_their_pads() {
    // Each hole 1.2mm from the middle of its land along the part's x. The
    // second part is turned 180 degrees, so its hole moves the other way: the
    // two holes are 2.54 - 2.4 = 0.14mm apart against a 0.5mm minimum, while
    // each land's ring is 2.0 - 1.2 - 0.4 = 0.4mm.
    let towards = faults(
        &mut two_parts(Some((1.2, 0.0)), Rotation::DEG_180),
        ViolationKind::HoleToHole,
    );
    assert_eq!(towards.len(), 1, "{towards:?}");
    // Between the holes, which are at 11.2 and 11.34.
    assert!((towards[0].0 - 11.27).abs() < 1e-6, "{towards:?}");
}

#[test]
fn control_the_same_offsets_turned_the_same_way_leave_the_holes_apart() {
    // Neither part turned: both holes 1.2mm to the right, still 2.54mm apart.
    let apart = faults(
        &mut two_parts(Some((1.2, 0.0)), Rotation::ZERO),
        ViolationKind::HoleToHole,
    );
    assert!(apart.is_empty(), "{apart:?}");
}

#[test]
fn control_holes_in_the_middle_of_their_lands_are_2_54mm_apart() {
    let middle = faults(
        &mut two_parts(None, Rotation::DEG_180),
        ViolationKind::HoleToHole,
    );
    assert!(middle.is_empty(), "{middle:?}");
}

/// One part at (1.0, 10), 1.0mm from the board's left edge, with one pad whose
/// hole is `offset` from its middle.
fn near_the_edge(offset: Option<(f64, f64)>) -> BoardWorld {
    let mut world = board(
        vec![pad("1", 0.0, PadShape::Rect, (2.0, 3.0), offset)],
        Rotation::ZERO,
    );
    let ecs = world.ecs_mut();
    for mut position in ecs.query::<&mut Position>().iter_mut(ecs) {
        *position = Position::from_mm(1.0, 10.0);
    }
    world
}

#[test]
fn a_hole_moved_towards_the_edge_is_nearer_the_edge_than_its_pad() {
    // The hole 0.5mm left of the middle is 0.5mm from the edge, its own edge
    // 0.1mm from it.
    let moved = faults(
        &mut near_the_edge(Some((-0.5, 0.0))),
        ViolationKind::HoleToEdge,
    );
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert!((moved[0].0 - 0.5).abs() < 1e-6, "{moved:?}");
    // The controls: in the middle, 1.0mm from the edge; and moved the other
    // way, 1.5mm.
    let middle = faults(&mut near_the_edge(None), ViolationKind::HoleToEdge);
    assert!(middle.is_empty(), "{middle:?}");
    let away = faults(
        &mut near_the_edge(Some((0.5, 0.0))),
        ViolationKind::HoleToEdge,
    );
    assert!(away.is_empty(), "{away:?}");
}

#[test]
fn a_slot_is_measured_at_its_ends_not_at_its_middle() {
    // A 4.0 by 2.0mm land with a 2.4 by 1.0mm slot along x: the bit travels
    // 0.7mm either side of the slot's middle. With the slot 0.9mm to the right
    // of the land's middle its right end is 1.6mm out, 0.4mm from the edge,
    // and the slot's edge 0.5mm further in than that is nothing: the ring is
    // under a tenth of a millimetre. Measured at the middle, 0.9mm out, it would
    // be 0.6mm.
    let mut land = pad("1", 0.0, PadShape::Rect, (4.0, 2.0), Some((0.9, 0.0)));
    land.drill = Some(Nm::from_mm(1.0));
    land.slot = Some((Nm::from_mm(2.4), Nm::from_mm(1.0)));
    let narrow = rings(vec![land.clone()], Rotation::ZERO);
    assert!(at(&narrow, (10.9, 10.0)), "{narrow:?}");

    // The control: the same slot in the middle of the land is not near an edge.
    land.hole_offset = None;
    let middle = rings(vec![land], Rotation::ZERO);
    assert!(middle.is_empty(), "{middle:?}");
}
