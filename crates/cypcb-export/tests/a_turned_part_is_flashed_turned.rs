//! A part turned a quarter turn has its pads flashed turned with it.
//!
//! `cargo test -p cypcb-export --test a_turned_part_is_flashed_turned`
//!
//! The copper, mask and paste writers placed each pad where the turn put it
//! and flashed it with the aperture of the unturned part. An 0805 turned 90
//! degrees has its two pads one above the other, each 1.45 wide and 1.0 tall;
//! the files flashed them 1.0 wide and 1.45 tall, standing along the line
//! between them. The checker swapped the two sides and measured the right
//! board - so the board it passed was not the board the fab was sent.
//!
//! The control is the same part placed square: its flashes must not change.
//! The drill side of the same question - a slot turned with its part - is
//! `the_drill_file_mills_a_slot::the_slot_turns_with_the_part`.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::{
    export_copper_layer, export_soldermask, export_solderpaste, MaskPasteConfig, Side,
};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{
    bottom_name, mirrored_to_bottom, Footprint, FootprintLibrary, PadDef,
};
use cypcb_world::BoardWorld;

const FORMAT: CoordinateFormat = CoordinateFormat::FORMAT_MM_2_6;

/// An 0805 land: two pads 1.0 wide and 1.45 tall, 1.9 apart along x.
fn chip() -> Footprint {
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
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.9), Nm::from_mm(1.45)));
    Footprint {
        name: "CHIP".to_string(),
        description: String::new(),
        pads: vec![pad("1", -0.95), pad("2", 0.95)],
        bounds: body,
        courtyard: body,
        silk: Vec::new(),
    }
}

/// R1 square at (10, 10), R2 turned 90 at (20, 10), R3 turned 90 on the
/// bottom at (20, 4).
fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    let chip = chip();
    let mut flipped = mirrored_to_bottom(&chip);
    flipped.name = bottom_name("CHIP");
    library.register(chip);
    library.register(flipped);
    for (refdes, x, y, rotation, footprint) in [
        ("R1", 10.0, 10.0, Rotation::ZERO, "CHIP".to_string()),
        ("R2", 20.0, 10.0, Rotation::DEG_90, "CHIP".to_string()),
        ("R3", 20.0, 4.0, Rotation::DEG_90, bottom_name("CHIP")),
    ] {
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("10k"),
            Position::from_mm(x, y),
            rotation,
            FootprintRef::new(&footprint),
            NetConnections::new(),
        );
    }
    world.set_footprints(library.clone());
    (world, library)
}

/// Every flash in a Gerber file: its centre in mm and its aperture's
/// definition, `R,1.450000X1.000000`.
fn flashes(gerber: &str) -> Vec<((f64, f64), String)> {
    let mut apertures = std::collections::HashMap::new();
    let mut current = String::new();
    let mut out = Vec::new();
    for line in gerber.lines() {
        if let Some(rest) = line.strip_prefix("%ADD") {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let shape = rest[digits.len()..].trim_end_matches("*%").to_string();
            apertures.insert(digits, shape);
        } else if let Some(code) = line.strip_prefix('D').and_then(|l| l.strip_suffix('*')) {
            current = apertures.get(code).cloned().unwrap_or_default();
        } else if let Some(xy) = line.strip_suffix("D03*") {
            let (x, y) = xy[1..].split_once('Y').expect("a flash states X and Y");
            let mm = |v: &str| v.parse::<f64>().unwrap() / 1e6;
            out.push(((mm(x), mm(y)), current.clone()));
        }
    }
    out
}

/// The aperture each flash at `x` has, one per flash, in file order.
fn apertures_at_x(gerber: &str, x: f64, y_range: (f64, f64)) -> Vec<String> {
    flashes(gerber)
        .into_iter()
        .filter(|((fx, fy), _)| (fx - x).abs() < 1e-6 && *fy > y_range.0 && *fy < y_range.1)
        .map(|(_, shape)| shape)
        .collect()
}

fn copper(layer: Layer) -> String {
    let (mut world, library) = board();
    export_copper_layer(&mut world, &library, layer, &FORMAT).unwrap()
}

fn mask(side: Side) -> String {
    let (mut world, library) = board();
    export_soldermask(
        &mut world,
        &library,
        side,
        &FORMAT,
        &MaskPasteConfig::default(),
    )
    .unwrap()
}

fn paste(side: Side) -> String {
    let (mut world, library) = board();
    export_solderpaste(
        &mut world,
        &library,
        side,
        &FORMAT,
        &MaskPasteConfig::default(),
    )
    .unwrap()
}

// R2's two pads sit one above the other at x = 20, y = 10 -/+ 0.95.
const R2: (f64, (f64, f64)) = (20.0, (8.0, 12.0));
// R3's sit at x = 20, y = 4 -/+ 0.95, on the bottom.
const R3: (f64, (f64, f64)) = (20.0, (2.0, 6.0));

#[test]
fn a_part_placed_square_keeps_its_pads_as_drawn() {
    // The control: nothing about an unturned part changes.
    let file = copper(Layer::TopCopper);
    let r1: Vec<_> = flashes(&file)
        .into_iter()
        .filter(|((x, _), _)| *x < 15.0)
        .collect();
    assert_eq!(
        r1,
        vec![
            ((9.05, 10.0), "R,1.000000X1.450000".to_string()),
            ((10.95, 10.0), "R,1.000000X1.450000".to_string()),
        ],
        "{file}"
    );
}

#[test]
fn a_turned_part_is_flashed_turned_on_the_top_copper() {
    let file = copper(Layer::TopCopper);
    assert_eq!(
        apertures_at_x(&file, R2.0, R2.1),
        vec!["R,1.450000X1.000000"; 2],
        "{file}"
    );
}

#[test]
fn a_turned_part_opens_the_mask_turned() {
    // The board's expansion, 0.05 on each side, around the turned pad.
    let file = mask(Side::Top);
    assert_eq!(
        apertures_at_x(&file, R2.0, R2.1),
        vec!["R,1.550000X1.100000"; 2],
        "{file}"
    );
}

#[test]
fn a_turned_part_gets_its_paste_turned() {
    let file = paste(Side::Top);
    assert_eq!(
        apertures_at_x(&file, R2.0, R2.1),
        vec!["R,1.450000X1.000000"; 2],
        "{file}"
    );
}

#[test]
fn a_turned_part_on_the_bottom_is_flashed_turned_on_every_bottom_file() {
    let bottom_copper = copper(Layer::BottomCopper);
    assert_eq!(
        apertures_at_x(&bottom_copper, R3.0, R3.1),
        vec!["R,1.450000X1.000000"; 2],
        "{bottom_copper}"
    );
    let bottom_mask = mask(Side::Bottom);
    assert_eq!(
        apertures_at_x(&bottom_mask, R3.0, R3.1),
        vec!["R,1.550000X1.100000"; 2],
        "{bottom_mask}"
    );
    let bottom_paste = paste(Side::Bottom);
    assert_eq!(
        apertures_at_x(&bottom_paste, R3.0, R3.1),
        vec!["R,1.450000X1.000000"; 2],
        "{bottom_paste}"
    );
}
