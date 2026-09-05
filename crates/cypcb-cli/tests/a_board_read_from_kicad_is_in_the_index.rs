//! A board read from KiCad is in the spatial index, and the rules can see it.
//!
//! `cargo test -p cypcb-cli --test a_board_read_from_kicad_is_in_the_index`
//!
//! Every rule that pairs one thing with another walks `world.spatial()`.
//! `parse_kicad_pcb` filled the world with components, nets and footprints and
//! left that index empty, so `ClearanceRule` on an imported board compared no
//! pairs at all and called it clean - on every board, every time. The
//! command-line path filled the index itself, so the shipped `cypcb check` was
//! never blind; anything holding this crate directly was, and a measurement
//! written a day earlier in this repository was: it counted zero clearance
//! violations and concluded the boards were clean, which was true for a reason
//! it had not established.
//!
//! So the index is filled where the board is read, and the case that matters
//! most here is the positive control: a check that finds nothing is worth
//! nothing until the same check, on the same board, is shown finding
//! something.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::rules::ClearanceRule;
use cypcb_drc::{DesignRules, DrcRule};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
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

/// The KiCad boards in this repository this reader can read, and how many
/// entries each one puts in the index.
const BOARDS: [(&str, usize); 4] = [
    (
        "tests/fixtures/kicad-tools/boards/03-usb-joystick/usb_joystick_routed.kicad_pcb",
        12,
    ),
    (
        "tests/fixtures/kicad-tools/boards/01-voltage-divider/voltage_divider.kicad_pcb",
        4,
    ),
    (
        "tests/fixtures/kicad-tools/boards/02-charlieplex-led/charlieplex_3x3.kicad_pcb",
        14,
    ),
    (
        "tests/fixtures/kicad-tools/tests/fixtures/routing-diagnostic.kicad_pcb",
        4,
    ),
];

#[test]
fn a_board_arrives_with_its_parts_in_the_index() {
    for (board, entries) in BOARDS {
        let parsed = cypcb_kicad::parse_kicad_pcb(&repo_root().join(board))
            .unwrap_or_else(|error| panic!("{board}: {error}"));
        assert_eq!(
            parsed.world.spatial().iter().count(),
            entries,
            "{board} came back with an index that does not hold its parts, so \
             every rule that pairs two things sees an empty board"
        );
    }
}

#[test]
fn the_rules_can_find_something_on_the_board_they_call_clean() {
    // The positive control. `ClearanceRule` reports nothing on these boards,
    // and that means nothing until the same rule, on the same board, is made
    // to report: here by asking for 20mm of clearance, which is wider than
    // any of these boards, so every pair of parts on them is too close.
    for (board, _) in BOARDS {
        let parsed = cypcb_kicad::parse_kicad_pcb(&repo_root().join(board))
            .unwrap_or_else(|error| panic!("{board}: {error}"));
        let mut world = parsed.world;

        let honest = DesignRules::jlcpcb_2layer();
        assert!(
            ClearanceRule.check(&mut world, &honest).is_empty(),
            "{board} is refused at the fab's own clearance"
        );

        let absurd = DesignRules {
            min_clearance: Nm::from_mm(20.0),
            ..honest
        };
        assert!(
            !ClearanceRule.check(&mut world, &absurd).is_empty(),
            "{board} passes 20mm of clearance between every pair of pads, which \
             is wider than the board - the rule is looking at an empty index"
        );
    }
}

#[test]
fn two_pads_with_no_net_are_compared_to_each_other() {
    // Copper with no net is copper. This was recorded a day earlier as a gap -
    // "a pad with no net is never compared to anything" - and it was the empty
    // index talking: the fixture that found it never reached the rule at all.
    // With the board in the index, the pair is measured like any other.
    let mut world = BoardWorld::new();
    world.set_board("d".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

    let mut library = FootprintLibrary::new();
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

    // 10 microns of copper-to-copper gap, on parts wired to nothing.
    for (refdes, x_mm) in [("R1", 5.0), ("R2", 5.61)] {
        world.spawn_component(
            RefDes::new(refdes),
            Value::new(""),
            Position::from_mm(x_mm, 5.0),
            Rotation::ZERO,
            FootprintRef::new("square"),
            NetConnections::new(),
        );
    }
    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);

    let violations = ClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());
    assert!(
        !violations.is_empty(),
        "two netless pads 10 microns apart were not compared to each other"
    );
}
