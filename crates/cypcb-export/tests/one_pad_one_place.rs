//! The copper and the drill file have to agree where a pad is.
//!
//! `cargo test -p cypcb-export --test one_pad_one_place`
//!
//! Turning a pad's own offset into a board coordinate is one multiplication,
//! and the workspace had **five** copies of it: two in `cypcb-autoroute`, one
//! in `cypcb-drc`, and two here. Two of them had already drifted - the copper
//! writer truncated the rotated offset toward zero where the drill writer
//! rounded it - so the same pad's flash and its own hole could land a
//! nanometre apart, and asymmetrically about the origin.
//!
//! Nothing caught it because every bundled board is placed at a multiple of 90
//! degrees, where the trigonometry lands on whole nanometres and truncating
//! and rounding give the same answer. This test places a part at 45 degrees,
//! which is where they do not: a 1mm offset lands on 707106.78nm, so one way
//! writes 707106 and the other 707107.
//!
//! There is one definition now, in `cypcb-world` beside the pads it places.
//! This is the guard that would have caught the drift.

use cypcb_core::{Nm, Point};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::export_excellon;
use cypcb_export::gerber::export_copper_layer;
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A board with one through-hole pad, 1mm out along the part's own x, on a
/// part placed at 10,10 and turned 45 degrees.
fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "angled".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        2,
    );

    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(Footprint {
        name: "pin".to_string(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Circle,
            position: Point::from_mm(1.0, 0.0),
            size: (Nm::from_mm(1.8), Nm::from_mm(1.8)),
            drill: Some(Nm::from_mm(0.9)),
            slot: None,
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
        }],
        ..base
    });
    world.set_footprints(library.clone());

    world.spawn_component(
        RefDes::new("J1"),
        Value::new(""),
        Position::from_mm(10.0, 10.0),
        Rotation::from_degrees(45.0),
        FootprintRef::new("pin"),
        NetConnections::new(),
    );
    (world, library)
}

/// 1mm turned 45 degrees is 707106.78nm on each axis, which rounds to 707107.
const EXPECTED_NM: i64 = 10_707_107;
/// What truncating toward zero would have written instead.
const TRUNCATED_NM: i64 = 10_707_106;

#[test]
fn the_flash_and_the_hole_are_at_the_same_point() {
    let (mut world, library) = board();

    let copper = export_copper_layer(
        &mut world,
        &library,
        Layer::TopCopper,
        &CoordinateFormat::FORMAT_MM_2_6,
    )
    .expect("the copper exports");
    let drill = export_excellon(&mut world, &library, &CoordinateFormat::FORMAT_MM_2_6, None)
        .expect("the drill file exports");

    assert!(
        copper.contains(&format!("X{EXPECTED_NM}Y{EXPECTED_NM}")),
        "the flash is where the model says: {}",
        copper
            .lines()
            .filter(|line| line.contains("D03"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        drill.contains("X10.707107Y10.707107"),
        "and so is the hole: {}",
        drill
            .lines()
            .filter(|line| line.starts_with('X'))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn neither_file_truncates_toward_the_origin() {
    // The specific defect: truncation is a bias rather than a rounding rule,
    // and it points at the origin - so it moves a pad the wrong way by up to a
    // nanometre, in a direction that depends on which quadrant it is in.
    let (mut world, library) = board();

    let copper = export_copper_layer(
        &mut world,
        &library,
        Layer::TopCopper,
        &CoordinateFormat::FORMAT_MM_2_6,
    )
    .expect("the copper exports");

    assert!(!copper.contains(&format!("X{TRUNCATED_NM}")), "{copper}");
}
