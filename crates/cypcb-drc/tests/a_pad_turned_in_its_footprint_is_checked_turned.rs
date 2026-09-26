//! The checker measures a pad turned inside its footprint as it stands.
//!
//! `cargo test -p cypcb-drc --test a_pad_turned_in_its_footprint_is_checked_turned`
//!
//! A pad's own turn adds to its part's: a pad turned a quarter on a square
//! part stands as a square pad on a part turned a quarter. With the pad at
//! its footprint's origin the two put it at the same centre, so the checker
//! must say the same about both boards - here two parts on two nets, 2.95
//! apart, whose pads are 2.9 wide along x once turned and 1.0 when not. The
//! footprint's courtyard is square, so a turn of the part does not move it.
//!
//! The control is the square board, where the same pads leave 1.95 between
//! them and the mask and paste rules are quiet: a check that cannot see the
//! turn would say the same about all three. The copper rule's own half,
//! `pad_copper`, is pinned beside it in `clearance.rs`.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::{run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, PinConnection, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// Two parts at x = 10 and 12.95, turned `part`, each holding one pad 1.0 wide
/// and 2.9 tall at its origin, turned `pad`, on a net of its own.
fn board(part: Rotation, pad: Rotation) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    let square = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.9), Nm::from_mm(2.9)));
    library.register_design(Footprint {
        name: "land".to_string(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.0), Nm::from_mm(2.9)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper, Layer::TopMask, Layer::TopPaste],
            mask_margin: None,
            rotation: pad,
        }],
        description: String::new(),
        bounds: square,
        courtyard: square,
        silk: Vec::new(),
    });
    world.set_footprints(library);
    for (refdes, x, net) in [("R1", 10.0, "A"), ("R2", 12.95, "B")] {
        let net = world.intern_net(net);
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", net));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("10k"),
            Position::from_mm(x, 10.0),
            part,
            FootprintRef::new("land"),
            nets,
        );
    }
    world
}

/// What the checker says about a board: each fault's kind and where.
fn verdict(part: Rotation, pad: Rotation) -> Vec<(ViolationKind, Point)> {
    let mut faults: Vec<_> = run_drc(&mut board(part, pad), &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .map(|violation| (violation.kind, violation.location))
        .collect();
    faults.sort_by_key(|(kind, at)| (format!("{kind:?}"), at.x.0, at.y.0));
    faults
}

#[test]
fn a_pad_turned_in_its_footprint_is_checked_as_its_part_turned() {
    assert_eq!(
        verdict(Rotation::ZERO, Rotation::DEG_90),
        verdict(Rotation::DEG_90, Rotation::ZERO)
    );
}

#[test]
fn turned_pads_this_close_bridge_the_mask_and_tear_the_stencil() {
    let faults = verdict(Rotation::ZERO, Rotation::DEG_90);
    for kind in [
        ViolationKind::SolderMaskBridge,
        ViolationKind::PasteClearance,
    ] {
        assert!(
            faults.iter().any(|(k, _)| *k == kind),
            "{kind:?}: {faults:?}"
        );
    }
}

#[test]
fn the_same_pads_square_are_far_enough_apart() {
    let faults = verdict(Rotation::ZERO, Rotation::ZERO);
    for kind in [
        ViolationKind::SolderMaskBridge,
        ViolationKind::PasteClearance,
    ] {
        assert!(
            !faults.iter().any(|(k, _)| *k == kind),
            "{kind:?}: {faults:?}"
        );
    }
}
