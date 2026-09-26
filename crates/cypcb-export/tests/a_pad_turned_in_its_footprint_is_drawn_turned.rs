//! A pad turned inside its footprint is drawn turned in every file.
//!
//! `cargo test -p cypcb-export --test a_pad_turned_in_its_footprint_is_drawn_turned`
//!
//! A footprint can hold a pad across its own axes - a pin header drawn with
//! its pads long in y. The pad's own turn adds to its part's turn: a pad
//! turned a quarter on a square part lands the same as a square pad on a
//! part turned a quarter. With the pad at the footprint's origin both put it
//! at the same centre, so every file must come out the same for the two.
//!
//! The control is the same pad square on a square part: its files must
//! differ, or the comparison could not see a turn at all.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::export_excellon;
use cypcb_export::gerber::export_copper_layer;
use cypcb_export::ipc2581::{export_ipc2581, HouseTolerances};
use cypcb_export::ipc356::export_ipc356;
use cypcb_export::{dxf, pdf, svg};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, PinConnection, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

const FORMAT: CoordinateFormat = CoordinateFormat::FORMAT_MM_2_6;

/// One part at (10, 10) turned `part`, holding one 1.524 by 3.048 pad at its
/// origin turned `pad`, milled with a slot 1.0 by 2.4 along its length.
fn board(part: Rotation, pad: Rotation) -> (BoardWorld, FootprintLibrary) {
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(3.048), Nm::from_mm(3.048)));
    let footprint = Footprint {
        name: "HDR".to_string(),
        description: String::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.524), Nm::from_mm(3.048)),
            drill: Some(Nm::from_mm(1.0)),
            slot: Some((Nm::from_mm(1.0), Nm::from_mm(2.4))),
            layers: vec![Layer::TopCopper, Layer::BottomCopper, Layer::TopMask],
            mask_margin: None,
            rotation: pad,
        }],
        bounds: body,
        courtyard: body,
        silk: Vec::new(),
    };
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
    let net = world.intern_net("SIG");
    let mut library = FootprintLibrary::new();
    library.register(footprint);
    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1", net));
    world.spawn_component(
        RefDes::new("J1"),
        Value::new("HDR"),
        Position::from_mm(10.0, 10.0),
        part,
        FootprintRef::new("HDR"),
        nets,
    );
    world.set_footprints(library.clone());
    (world, library)
}

/// Every file this board is drawn in, by name.
fn files(part: Rotation, pad: Rotation) -> Vec<(&'static str, String)> {
    let (mut world, library) = board(part, pad);
    let copper = export_copper_layer(&mut world, &library, Layer::TopCopper, &FORMAT).unwrap();
    let copper = without_timestamp(&copper);
    let svg = svg::plot_layer(&mut world, &library, Layer::TopCopper);
    let pdf = pdf::plot_layer(&mut world, &library, Layer::TopCopper);
    let dxf = dxf::plot_layer(&mut world, &library, Layer::TopCopper);
    let (ipc2581, _) = export_ipc2581(&mut world, &library, HouseTolerances::default(), "T");
    let (ipc356, _) = export_ipc356(&mut world, &library, "t");
    let drill = export_excellon(&mut world, &library, &FORMAT, None).unwrap();
    vec![
        ("gerber copper", copper),
        ("svg", svg),
        ("pdf", pdf),
        ("dxf", dxf),
        ("ipc2581", ipc2581),
        ("ipc356", without_timestamp(&ipc356)),
        ("drill", without_timestamp(&drill)),
    ]
}

/// The file with every line that states when it was written taken out.
fn without_timestamp(file: &str) -> String {
    file.lines()
        .filter(|l| !l.contains("CreationDate") && !l.contains("DATE"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_pad_turned_in_its_footprint_is_drawn_as_its_part_turned() {
    let turned_pad = files(Rotation::ZERO, Rotation::DEG_90);
    let turned_part = files(Rotation::DEG_90, Rotation::ZERO);
    for ((name, pad), (_, part)) in turned_pad.iter().zip(&turned_part) {
        assert_eq!(
            pad, part,
            "{name}: a pad turned in its footprint drew differently"
        );
    }
}

#[test]
fn a_square_pad_on_a_square_part_is_drawn_differently() {
    let square = files(Rotation::ZERO, Rotation::ZERO);
    let turned_pad = files(Rotation::ZERO, Rotation::DEG_90);
    for ((name, square), (_, turned)) in square.iter().zip(&turned_pad) {
        assert_ne!(square, turned, "{name}: the files cannot see a pad's turn");
    }
}

#[test]
fn turns_of_the_pad_and_the_part_add_up() {
    let both = files(Rotation::DEG_90, Rotation::DEG_90);
    let half_turn = files(Rotation::DEG_180, Rotation::ZERO);
    for ((name, both), (_, half)) in both.iter().zip(&half_turn) {
        assert_eq!(
            both, half,
            "{name}: two quarter turns did not make a half turn"
        );
    }
}
