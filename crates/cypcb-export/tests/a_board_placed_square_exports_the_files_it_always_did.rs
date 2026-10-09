//! A board whose parts turn by multiples of 90 exports byte for byte what it
//! exported before an angle could reach the aperture.
//!
//! `cargo test -p cypcb-export --test a_board_placed_square_exports_the_files_it_always_did`
//!
//! Keeping the angle meant touching the one function every pad flash goes
//! through, so the boards that never had an angle - every example in this
//! repository turns its parts 0, 90, 180 or 270 - must come out unchanged.
//! The three files under `fixtures/square-turned-board/` were generated from
//! the code as it stood before the change, and this builds the same board -
//! rectangles, oblongs, a circle and a rounded rectangle, at 0, 90, 180 and
//! 270 - and holds every byte of its copper, mask and paste against them.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::{
    export_copper_layer, export_soldermask, export_solderpaste, MaskPasteConfig, Side,
};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

const FORMAT: CoordinateFormat = CoordinateFormat::FORMAT_MM_2_6;

fn pad(number: &str, x: f64, shape: PadShape, w: f64, h: f64) -> PadDef {
    PadDef {
        number: number.to_string(),
        shape,
        position: Point::from_mm(x, 0.0),
        size: (Nm::from_mm(w), Nm::from_mm(h)),
        drill: None,
        slot: None,
        layers: vec![Layer::TopCopper, Layer::TopMask, Layer::TopPaste],
        mask_margin: None,
        rotation: Rotation::ZERO,
    }
}

fn footprint(name: &str, shape: PadShape, w: f64, h: f64) -> Footprint {
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(w + 1.0), Nm::from_mm(h + 1.0)));
    Footprint {
        name: name.to_string(),
        description: String::new(),
        pads: vec![pad("1", -0.95, shape, w, h), pad("2", 0.95, shape, w, h)],
        bounds: body,
        courtyard: body,
        silk: Vec::new(),
    }
}

fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(50.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(footprint("CHIP", PadShape::Rect, 1.0, 1.45));
    library.register(footprint("OVAL", PadShape::Oblong, 2.4, 1.0));
    library.register(footprint("CIRC", PadShape::Circle, 1.6, 1.6));
    library.register(footprint(
        "ROUND",
        PadShape::RoundRect { corner_ratio: 25 },
        1.0,
        0.6,
    ));
    for (refdes, x, rotation, fp) in [
        ("R1", 10.0, Rotation::ZERO, "CHIP"),
        ("R2", 15.0, Rotation::DEG_90, "CHIP"),
        ("R3", 20.0, Rotation::DEG_180, "OVAL"),
        ("R4", 25.0, Rotation::DEG_270, "OVAL"),
        ("C1", 30.0, Rotation::ZERO, "CIRC"),
        ("K1", 35.0, Rotation::DEG_90, "ROUND"),
    ] {
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("v"),
            Position::from_mm(x, 10.0),
            rotation,
            FootprintRef::new(fp),
            NetConnections::new(),
        );
    }
    world.set_footprints(library.clone());
    (world, library)
}

#[test]
fn the_copper_is_byte_for_byte_the_file_before_the_angle() {
    let (mut world, library) = board();
    let copper = export_copper_layer(
        &mut world,
        &library,
        Layer::TopCopper,
        &FORMAT,
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(
        copper,
        include_str!("fixtures/square-turned-board/copper.gbr")
    );
}

#[test]
fn the_mask_is_byte_for_byte_the_file_before_the_angle() {
    let (mut world, library) = board();
    let mask = export_soldermask(
        &mut world,
        &library,
        Side::Top,
        &FORMAT,
        &MaskPasteConfig::default(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(mask, include_str!("fixtures/square-turned-board/mask.gbr"));
}

#[test]
fn the_paste_is_byte_for_byte_the_file_before_the_angle() {
    let (mut world, library) = board();
    let paste = export_solderpaste(
        &mut world,
        &library,
        Side::Top,
        &FORMAT,
        &MaskPasteConfig::default(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(
        paste,
        include_str!("fixtures/square-turned-board/paste.gbr")
    );
}
