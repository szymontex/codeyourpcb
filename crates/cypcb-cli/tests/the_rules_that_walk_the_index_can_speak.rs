//! Every rule that walks the spatial index, shown reporting something.
//!
//! `cargo test -p cypcb-cli --test the_rules_that_walk_the_index_can_speak`
//!
//! Four of this project's 37 rules pair entities through `world.spatial()`:
//! `clearance`, `edge-clearance`, `mounting-hole-clearance` and
//! `slot-clearance`. Until 2026-08-31 a board read by `parse_kicad_pcb` came
//! back with that index empty, so all four were silent on every imported
//! board - and silence read as a clean board.
//!
//! `clearance` has its control in `a_board_read_from_kicad_is_in_the_index`.
//! This file is the other three, because a rule that has never been seen
//! reporting is a rule nobody knows is connected.
//!
//! `slot-clearance` is controlled on a board built here rather than on one of
//! this repository's own: the KiCad fixtures that carry slots -
//! `slotted.kicad_pcb`, `kicad10-slotted.kicad_pcb`, `usb_c_named_pads
//! .kicad_pcb` - have no foreign copper anywhere near those slots, so the rule
//! is quiet on them for a reason that is about the boards rather than about
//! the rule. Measured before this file was written, at 20mm of demanded
//! clearance: still nothing.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::rules::{EdgeClearanceRule, MountingHoleClearanceRule, SlotClearanceRule};
use cypcb_drc::{DesignRules, DrcRule};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position, RefDes,
    Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two directories above this crate")
}

fn board(path: &str) -> BoardWorld {
    cypcb_kicad::parse_kicad_pcb(&repo_root().join(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
        .world
}

#[test]
fn the_edge_rule_speaks_on_a_board_that_is_too_close_to_its_own_edge() {
    // A real finding, and one the empty index hid: this board puts copper
    // inside JLCPCB's own edge clearance, once. Every other board here clears
    // it, and all of them are refused when 20mm is demanded - which is how the
    // silence above is shown to be a measurement rather than a dead rule.
    let honest = DesignRules::jlcpcb_2layer();
    let mut loud = honest.clone();
    loud.min_edge_clearance = Nm::from_mm(20.0);

    let mut diagnostic = board("viewer/kicad-tools/tests/fixtures/routing-diagnostic.kicad_pcb");
    assert_eq!(
        EdgeClearanceRule.check(&mut diagnostic, &honest).len(),
        1,
        "this board has one piece of copper inside the fab's edge clearance"
    );

    for path in [
        "viewer/kicad-tools/boards/03-usb-joystick/output/usb_joystick_routed.kicad_pcb",
        "viewer/kicad-tools/boards/01-voltage-divider/output/voltage_divider.kicad_pcb",
        "viewer/kicad-tools/boards/02-charlieplex-led/output/charlieplex_3x3.kicad_pcb",
    ] {
        let mut world = board(path);
        assert!(
            EdgeClearanceRule.check(&mut world, &honest).is_empty(),
            "{path} is refused at the fab's own edge clearance"
        );
        assert!(
            !EdgeClearanceRule.check(&mut world, &loud).is_empty(),
            "{path} keeps 20mm from its own edges, which no board does - the \
             rule is looking at an empty index"
        );
    }
}

#[test]
fn the_mounting_hole_rule_speaks_on_the_one_board_here_that_has_holes() {
    // `mcu_board` is the only KiCad board in this repository with mounting
    // holes. It passes the fab's figure and is refused when 20mm is demanded,
    // which is what proves the rule reaches this board's holes at all.
    let honest = DesignRules::jlcpcb_2layer();
    let mut loud = honest.clone();
    loud.min_edge_clearance = Nm::from_mm(20.0);

    let mut world =
        board("viewer/kicad-tools/examples/06-intelligent-placement/fixtures/mcu_board.kicad_pcb");
    assert!(
        MountingHoleClearanceRule
            .check(&mut world, &honest)
            .is_empty(),
        "this board is refused around its own mounting holes"
    );
    assert_eq!(
        MountingHoleClearanceRule.check(&mut world, &loud).len(),
        2,
        "the two mounting holes on this board are not being measured"
    );
}

#[test]
fn the_slot_rule_speaks_when_foreign_copper_is_beside_a_slot() {
    // The board this repository does not have: a part anchored by a milled
    // slot with somebody else's pad 0.15mm from it. The fixtures that carry
    // slots have nothing near them, so this is where the rule is shown to
    // work.
    let mut world = BoardWorld::new();
    world.set_board("s".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "anchor".to_string(),
        description: String::new(),
        bounds: Rect::from_points(Point::from_mm(-1.6, -0.9), Point::from_mm(1.6, 0.9)),
        courtyard: Rect::from_points(Point::from_mm(-2.0, -1.2), Point::from_mm(2.0, 1.2)),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Oblong,
            position: Point::ORIGIN,
            size: (Nm::from_mm(3.2), Nm::from_mm(1.8)),
            drill: Some(Nm::from_mm(1.0)),
            // Milled along its length: 2.4mm of travel on a 1.0mm bit, which
            // is how a USB shell or a latching header holds itself down.
            slot: Some((Nm::from_mm(2.4), Nm::from_mm(1.0))),
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
            mask_margin: None,
        }],
    });
    library.register(Footprint {
        name: "square".to_string(),
        description: String::new(),
        bounds: Rect::from_points(Point::from_mm(-0.3, -0.3), Point::from_mm(0.3, 0.3)),
        courtyard: Rect::from_points(Point::from_mm(-0.5, -0.5), Point::from_mm(0.5, 0.5)),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(0.6), Nm::from_mm(0.6)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        }],
    });

    let mut anchor_nets = NetConnections::new();
    anchor_nets.add(PinConnection::new("1".to_string(), NetId::new(1)));
    world.spawn_component(
        RefDes::new("J1"),
        Value::new(""),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("anchor"),
        anchor_nets,
    );

    // One neighbour close to the slot on its own net, one far away.
    for (refdes, x_mm, net) in [("R1", 11.5, 2u32), ("R2", 15.0, 3)] {
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1".to_string(), NetId::new(net)));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new(""),
            Position::from_mm(x_mm, 10.0),
            Rotation::ZERO,
            FootprintRef::new("square"),
            nets,
        );
    }
    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);

    let violations = SlotClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());
    assert_eq!(
        violations.len(),
        1,
        "the pad beside the slot is not reported, and the one across the board \
         should not be"
    );
}
