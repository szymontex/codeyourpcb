//! A hole too near the board edge comes out open on one side.
//!
//! `cargo test -p cypcb-drc --test a_hole_the_router_would_break_into`
//!
//! The board is cut out of a panel by a milling bit that follows the outline.
//! A drilled hole whose wall sits closer to that path than the fab allows is a
//! mounting hole with a notch, or a plated hole whose barrel is gone.
//!
//! `min_edge_clearance` does not answer this. That rule measures **copper**
//! against the edge, and a mounting hole has none at all - a drill with no
//! copper is invisible to it by construction, which is exactly the hole a
//! screw goes through at the edge of a board.
//!
//! `min_hole_to_edge` is one of fifteen numbers every fab preset published
//! with nothing in the workspace reading them.

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::components::trace::Via;
use cypcb_world::components::{
    FootprintRef, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A 20x20 board with one via, `x_mm` from the left edge.
fn board_with_via(x_mm: f64) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "edged".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    let net = world.intern_net("GND");
    let mut via = Via::new(Point::from_mm(x_mm, 10.0), net);
    via.drill = Nm::from_mm(0.4);
    world.ecs_mut().spawn((via, net));
    world
}

/// The same board with a mounting hole: a drill and no copper anywhere.
fn board_with_mounting_hole(x_mm: f64) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "edged".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );

    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(Footprint {
        name: "screw".to_string(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Circle,
            position: Point::from_mm(0.0, 0.0),
            // An M3 clearance hole with no copper: a screw goes through it and
            // nothing is soldered to it.
            size: (Nm::from_mm(3.2), Nm::from_mm(3.2)),
            drill: Some(Nm::from_mm(3.2)),
            slot: None,
            layers: Vec::new(),
            mask_margin: None,
        }],
        ..base
    });
    world.set_footprints(library);

    world.spawn_component(
        RefDes::new("H1"),
        Value::new(""),
        Position::from_mm(x_mm, 10.0),
        Rotation::ZERO,
        FootprintRef::new("screw"),
        NetConnections::new(),
    );
    world
}

fn edge_faults(world: &mut BoardWorld) -> Vec<String> {
    run_drc(world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| violation.kind == ViolationKind::HoleToEdge)
        .map(|violation| violation.message)
        .collect()
}

#[test]
fn a_via_the_bit_would_meet_is_reported() {
    // 0.4mm hole centred 0.4mm from the edge: its wall is 0.2mm from the cut,
    // and the fab wants 0.3mm.
    let faults = edge_faults(&mut board_with_via(0.4));

    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("0.200mm") && faults[0].contains("0.300mm"),
        "the message carries both numbers: {}",
        faults[0]
    );
}

#[test]
fn a_via_well_inside_the_board_says_nothing() {
    let faults = edge_faults(&mut board_with_via(10.0));

    assert_eq!(faults, Vec::<String>::new());
}

#[test]
fn the_wall_is_measured_rather_than_the_centre() {
    // Centred 0.5mm in, the same via's wall is 0.3mm from the cut - exactly
    // what the fab allows, and it must not be reported. A rule measuring
    // centres would call this 0.5mm and pass a board it should not.
    let faults = edge_faults(&mut board_with_via(0.5));

    assert_eq!(faults, Vec::<String>::new());
    // And a hair closer is a fault, so the boundary is where it says it is.
    assert_eq!(edge_faults(&mut board_with_via(0.49)).len(), 1);
}

#[test]
fn the_copper_rule_cannot_answer_this_one() {
    // A mounting hole is a drill with no copper at all, so there is nothing
    // for a rule about copper to measure - it is invisible to edge clearance
    // by construction. Put a 3.2mm one 1.7mm from the edge and its wall is
    // 0.1mm from the cut: the screw hole comes out as a notch.
    //
    // The first version of this test used a hole inside a wide pad, which
    // cannot show anything: a concentric hole is always further from the edge
    // than the copper around it, so the copper rule always fires first.
    let mut world = board_with_mounting_hole(1.7);

    let copper = run_drc(&mut world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| violation.kind == ViolationKind::EdgeClearance)
        .count();
    let holes = edge_faults(&mut board_with_mounting_hole(1.7));

    assert_eq!(copper, 0, "there is no copper to measure");
    assert_eq!(holes.len(), 1, "and the hole is too close: {holes:?}");
    assert!(holes[0].contains("0.100mm"), "{}", holes[0]);
}

#[test]
fn every_preset_publishes_a_number_for_this() {
    for preset in Preset::all() {
        assert!(
            preset.rules().min_hole_to_edge > Nm(0),
            "{} states no hole-to-edge distance",
            preset.name()
        );
    }
}
