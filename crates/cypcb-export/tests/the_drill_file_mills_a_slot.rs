//! A slot is milled from one end to the other, and the file has to say so.
//!
//! `cargo test -p cypcb-export --test the_drill_file_mills_a_slot`
//!
//! Excellon has one way to order a slot: put the bit down at one end centre
//! and drive it to the other, written `X..Y..G85X..Y..`. A file that gives
//! only the first point orders a round hole of the bit's diameter - so a USB
//! connector's 2.4x1.0mm slot arrives as a 1mm hole, the part does not fit,
//! and the board is scrap. Nothing in the file says anything went wrong.
//!
//! The bit is the narrow dimension and it stops half a bit short of each end,
//! so the travel is `long - narrow`: a 2.4x1.0mm slot is a 1mm tool moving
//! 1.4mm. That arithmetic is what the first test measures.

use cypcb_core::{Nm, Point};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::export_excellon;
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A board with one part whose single pad has the given hole.
fn board_with_hole(
    drill: Nm,
    slot: Option<(Nm, Nm)>,
    rotation: Rotation,
) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "slotted".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        2,
    );

    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(Footprint {
        name: "jack".to_string(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Oblong,
            position: Point::ORIGIN,
            size: (Nm::from_mm(3.2), Nm::from_mm(1.8)),
            drill: Some(drill),
            slot,
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
            mask_margin: None,
        }],
        ..base
    });
    world.set_footprints(library.clone());

    world.spawn_component(
        RefDes::new("J1"),
        Value::new(""),
        Position::from_mm(10.0, 10.0),
        rotation,
        FootprintRef::new("jack"),
        NetConnections::new(),
    );
    (world, library)
}

fn drill_file(drill: Nm, slot: Option<(Nm, Nm)>, rotation: Rotation) -> String {
    let (mut world, library) = board_with_hole(drill, slot, rotation);
    export_excellon(&mut world, &library, &CoordinateFormat::FORMAT_MM_2_6, None)
        .expect("the board exports")
}

#[test]
fn the_bit_travels_the_length_of_the_slot() {
    // 2.4mm long, 1.0mm wide, centred at 10,10: a 1mm tool from 9.3 to 10.7.
    let file = drill_file(
        Nm::from_mm(1.0),
        Some((Nm::from_mm(2.4), Nm::from_mm(1.0))),
        Rotation::ZERO,
    );

    assert!(
        file.contains("X9.300000Y10.000000G85X10.700000Y10.000000"),
        "the slot is one milled path:\n{file}"
    );
    assert!(
        file.contains("T1C1.000000"),
        "and the tool is the narrow dimension, not the length:\n{file}"
    );
}

#[test]
fn the_slot_turns_with_the_part() {
    // The same slot on a part rotated 90 degrees runs along y instead.
    let file = drill_file(
        Nm::from_mm(1.0),
        Some((Nm::from_mm(2.4), Nm::from_mm(1.0))),
        Rotation::from_degrees(90.0),
    );

    assert!(
        file.contains("X10.000000Y9.300000G85X10.000000Y10.700000"),
        "a rotated slot is milled along the axis the part turned it to:\n{file}"
    );
}

#[test]
fn a_round_hole_is_one_hit() {
    // The control: no G85 anywhere, because a drill makes this hole in one go
    // and a routing path for it would be a slower board and a stranger file.
    let file = drill_file(Nm::from_mm(0.9), None, Rotation::ZERO);

    assert!(
        !file.contains("G85"),
        "a drilled hole is not milled:\n{file}"
    );
    assert!(file.contains("X10.000000Y10.000000"));
}

#[test]
fn a_square_oval_is_a_round_hole() {
    // `(drill oval 1.0 1.0)` is legal and means a 1mm drill. Milling it would
    // send the fab a zero-length path for a hole one hit makes.
    let file = drill_file(
        Nm::from_mm(1.0),
        Some((Nm::from_mm(1.0), Nm::from_mm(1.0))),
        Rotation::ZERO,
    );

    assert!(!file.contains("G85"), "{file}");
}
