//! The checker measures a turned part's openings as the fab files cut them.
//!
//! `cargo test -p cypcb-drc --test a_turned_part_is_checked_turned`
//!
//! The mask and paste rules swapped a pad's sides for a part turned a quarter
//! turn, each with its own copy of the test for a quarter turn, while the
//! Gerber writers did not swap them at all. Both now read
//! `PadDef::outline`, the one place a pad is turned. These tests pin the
//! checker's half: two parts turned 90 degrees side by side, close enough
//! that their turned pads - 1.45 wide along x - leave too thin a web, while
//! the same pads unturned - 1.0 wide - would leave plenty.

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// Two parts with an 0805 land - pads 1.0 wide and 1.45 tall, 1.9 apart along
/// x - at x = 10 and x = 10 + `spacing`, both turned `rotation`.
fn board(spacing_mm: f64, rotation: Rotation) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let pad = |number: &str, x: f64| PadDef {
        number: number.to_string(),
        shape: PadShape::Rect,
        position: Point::from_mm(x, 0.0),
        size: (Nm::from_mm(1.0), Nm::from_mm(1.45)),
        drill: None,
        slot: None,
        layers: vec![Layer::TopCopper, Layer::TopMask, Layer::TopPaste],
        mask_margin: None,
        rotation: Rotation::ZERO,
    };
    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(Footprint {
        name: "chip".to_string(),
        pads: vec![pad("1", -0.95), pad("2", 0.95)],
        ..base
    });
    world.set_footprints(library);
    for (refdes, x) in [("R1", 10.0), ("R2", 10.0 + spacing_mm)] {
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("10k"),
            Position::from_mm(x, 10.0),
            rotation,
            FootprintRef::new("chip"),
            NetConnections::new(),
        );
    }
    world
}

fn faults(world: &mut BoardWorld, kind: ViolationKind) -> usize {
    run_drc(world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| violation.kind == kind)
        .count()
}

#[test]
fn turned_pads_side_by_side_tear_the_stencil() {
    // 1.5 apart, turned: 1.5 - 1.45 leaves a web far under the 0.127 JLCPCB
    // publishes. Unturned it would be 1.5 - 1.0.
    assert!(
        faults(
            &mut board(1.5, Rotation::DEG_90),
            ViolationKind::PasteClearance
        ) > 0
    );
}

#[test]
fn turned_pads_side_by_side_bridge_the_mask() {
    // The same pair: the two turned openings, each grown by the board's mask
    // expansion, run into each other.
    assert!(
        faults(
            &mut board(1.5, Rotation::DEG_90),
            ViolationKind::SolderMaskBridge
        ) > 0
    );
}

#[test]
fn turned_pads_far_enough_apart_are_fine() {
    // The control on the rule firing at all: 3.0 apart leaves 1.55 between
    // the turned pads.
    let mut world = board(3.0, Rotation::DEG_90);
    assert_eq!(faults(&mut world, ViolationKind::PasteClearance), 0);
    assert_eq!(faults(&mut world, ViolationKind::SolderMaskBridge), 0);
}

#[test]
fn unturned_pads_are_measured_as_drawn() {
    // The control on the width: unturned, 1.05 apart leaves 1.05 - 1.0.
    let mut world = board(1.05, Rotation::ZERO);
    assert!(faults(&mut world, ViolationKind::PasteClearance) > 0);
    assert!(faults(&mut world, ViolationKind::SolderMaskBridge) > 0);
}
