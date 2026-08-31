//! A slot is as close to things as its ends are, not as its centre is.
//!
//! `cargo test -p cypcb-drc --test a_slot_is_measured_end_to_end`
//!
//! Every rule that asks about a hole treated it as a circle of the pad's drill
//! number, which for a slot is its **narrow** dimension. A 2.4x1.0mm slot was
//! measured as a 1mm circle at its centre - so it was wrong by up to 0.7mm in
//! the direction it is long, which is exactly the direction the next hole
//! usually is.
//!
//! The board that follows is what that costs: two slots with 0.4mm of laminate
//! between their ends, on a process that wants 0.5mm, reported as 1.8mm apart
//! and passed. The fab breaks through between them and the two holes come out
//! as one.
//!
//! The shape is a capsule now - a segment with a radius - and a drilled hole
//! is the same capsule with its two ends in the same place, so nothing about
//! a round hole changed.

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::components::{
    FootprintRef, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A 20x20 board carrying one slotted part per given position.
///
/// The slot is 2.4mm long and 1.0mm wide, so its capsule is a 1.4mm segment
/// with a 0.5mm radius: the bit centres sit 0.7mm either side of the pad.
fn board_with_slots(positions: &[(f64, f64)]) -> BoardWorld {
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
            layers: vec![
                cypcb_world::components::Layer::TopCopper,
                cypcb_world::components::Layer::BottomCopper,
            ],
            mask_margin: None,
        }],
        ..base
    });
    world.set_footprints(library);

    for (index, (x, y)) in positions.iter().enumerate() {
        world.spawn_component(
            RefDes::new(format!("J{}", index + 1)),
            Value::new(""),
            Position::from_mm(*x, *y),
            Rotation::ZERO,
            FootprintRef::new("latch"),
            NetConnections::new(),
        );
    }
    world
}

fn faults(world: &mut BoardWorld, kind: ViolationKind) -> Vec<String> {
    run_drc(world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| violation.kind == kind)
        .map(|violation| violation.message)
        .collect()
}

#[test]
fn two_slots_end_to_end_are_as_close_as_their_ends() {
    // Centres 2.8mm apart. The near ends are at 10.7 and 12.1, so 1.4mm of
    // centre-to-centre becomes 0.4mm of laminate once each 0.5mm wall is taken
    // off - under the 0.5mm this process wants.
    //
    // Read as two 1mm circles the same board says 1.8mm, which is comfortable,
    // and that is the number this used to report.
    let faults = faults(
        &mut board_with_slots(&[(10.0, 10.0), (12.8, 10.0)]),
        ViolationKind::HoleToHole,
    );

    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("0.40") || faults[0].contains("0.4"),
        "the gap reported is between the walls: {}",
        faults[0]
    );
}

#[test]
fn two_slots_side_by_side_keep_their_distance() {
    // The control, and the half that must not change: the same two slots
    // offset across their length rather than along it are 2.8mm apart in a
    // direction neither of them is long, so nothing is reported.
    let faults = faults(
        &mut board_with_slots(&[(10.0, 10.0), (10.0, 12.8)]),
        ViolationKind::HoleToHole,
    );

    assert_eq!(faults, Vec::<String>::new());
}

#[test]
fn a_slot_reaching_for_the_edge_is_reported() {
    // Centred 1.3mm in, so the centre clears the fab's 0.3mm easily. The
    // slot's own end is at 0.6mm and its wall at 0.1mm, which does not.
    let faults = faults(
        &mut board_with_slots(&[(1.3, 10.0)]),
        ViolationKind::HoleToEdge,
    );

    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(faults[0].contains("0.100mm"), "{}", faults[0]);
}

#[test]
fn a_slot_well_inside_the_board_says_nothing() {
    let faults = faults(
        &mut board_with_slots(&[(10.0, 10.0)]),
        ViolationKind::HoleToEdge,
    );

    assert_eq!(faults, Vec::<String>::new());
}

#[test]
fn the_depth_of_a_slot_is_its_narrow_dimension() {
    // A slot's length is a milling distance, not a depth: the plating has to
    // reach down 1.0mm here, not 2.4mm. Through the standard 1.6mm board that
    // is 1.6:1, well inside the 8:1 this fab plates.
    let faults = faults(
        &mut board_with_slots(&[(10.0, 10.0)]),
        ViolationKind::DrillAspectRatio,
    );

    assert_eq!(faults, Vec::<String>::new());
}
