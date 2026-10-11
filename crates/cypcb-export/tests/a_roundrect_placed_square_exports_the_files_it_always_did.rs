//! A rounded rectangle pad on a part turned by a multiple of 90 degrees
//! exports byte for byte what it exported before the slope reached it.
//!
//! `cargo test -p cypcb-export --test a_roundrect_placed_square_exports_the_files_it_always_did`
//!
//! A rounded rectangle stood at 30 degrees used to be flashed with its sides
//! along the board's axes. Giving it a slope touches the function every pad
//! flash goes through, so the pads that have no slope - a quarter turn only
//! swaps the sides - must come out as they did. The three files under
//! `fixtures/square-turned-roundrects/` were generated from the code as it
//! stood before the change; this builds the same board - a wide and a tall
//! rounded rectangle at 0, 90, 180 and 270, two corner ratios, one with a mask
//! margin of its own - and holds every byte of its copper, mask and paste
//! against them.

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

fn footprint(name: &str, w: f64, h: f64, ratio: u8, margin: Option<f64>) -> Footprint {
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(w + 1.0), Nm::from_mm(h + 1.0)));
    Footprint {
        name: name.to_string(),
        description: String::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::RoundRect {
                corner_ratio: ratio,
            },
            position: Point::ORIGIN,
            size: (Nm::from_mm(w), Nm::from_mm(h)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper, Layer::TopMask, Layer::TopPaste],
            mask_margin: margin.map(Nm::from_mm),
            rotation: Rotation::ZERO,
        }],
        bounds: body,
        courtyard: body,
        silk: Vec::new(),
    }
}

fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(60.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(footprint("WIDE", 2.0, 1.0, 25, None));
    library.register(footprint("TALL", 1.0, 2.0, 40, Some(0.1)));
    let mut x = 5.0;
    for (name, fp) in [("W", "WIDE"), ("T", "TALL")] {
        for (i, rotation) in [
            Rotation::ZERO,
            Rotation::DEG_90,
            Rotation::DEG_180,
            Rotation::DEG_270,
        ]
        .into_iter()
        .enumerate()
        {
            world.spawn_component(
                RefDes::new(format!("{name}{i}")),
                Value::new("v"),
                Position::from_mm(x, 10.0),
                rotation,
                FootprintRef::new(fp),
                NetConnections::new(),
            );
            x += 6.0;
        }
    }
    world.set_footprints(library.clone());
    (world, library)
}

fn copper() -> String {
    let (mut world, library) = board();
    export_copper_layer(
        &mut world,
        &library,
        Layer::TopCopper,
        &FORMAT,
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .unwrap()
}

fn mask() -> String {
    let (mut world, library) = board();
    export_soldermask(
        &mut world,
        &library,
        Side::Top,
        &FORMAT,
        &MaskPasteConfig::default(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .unwrap()
}

fn paste() -> String {
    let (mut world, library) = board();
    export_solderpaste(
        &mut world,
        &library,
        Side::Top,
        &FORMAT,
        &MaskPasteConfig::default(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .unwrap()
}

#[test]
fn the_copper_is_byte_for_byte_the_file_before_the_slope() {
    assert_eq!(
        copper(),
        include_str!("fixtures/square-turned-roundrects/copper.gbr")
    );
}

#[test]
fn the_mask_is_byte_for_byte_the_file_before_the_slope() {
    assert_eq!(
        mask(),
        include_str!("fixtures/square-turned-roundrects/mask.gbr")
    );
}

#[test]
fn the_paste_is_byte_for_byte_the_file_before_the_slope() {
    assert_eq!(
        paste(),
        include_str!("fixtures/square-turned-roundrects/paste.gbr")
    );
}
