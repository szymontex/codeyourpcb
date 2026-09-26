//! A through-hole pad is flashed on every copper layer it has copper on.
//!
//! `cargo test -p cypcb-export --test a_through_hole_pad_reaches_every_copper_file`
//!
//! The copper files and IPC-2581 asked a pad's layer list which layers it is
//! on, and the importer writes a plated through-hole pad as `TopCopper` and
//! `BottomCopper` because it does not know the layer count. On a four-layer
//! board the pin headers had no land on either inner layer in the Gerber, and
//! IPC-2581 wrote pads on the two faces only - while the checks now measure
//! the pad as copper on all four. The files and the checks read one answer,
//! `PadDef::copper_mask`.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::export_copper_layer;
use cypcb_export::ipc2581::{export_ipc2581, HouseTolerances};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, PinConnection, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

const FORMAT: CoordinateFormat = CoordinateFormat::FORMAT_MM_2_6;
const STACK: [(Layer, &str); 4] = [
    (Layer::TopCopper, "F_Cu"),
    (Layer::Inner(0), "In1_Cu"),
    (Layer::Inner(1), "In2_Cu"),
    (Layer::BottomCopper, "B_Cu"),
];

/// A four-layer board with one pad at 10mm, 10mm: drilled through the board,
/// or a top-side SMD pad of the same size.
fn board(through_hole: bool) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 4);
    let size = Nm::from_mm(1.7);
    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "PAD".to_string(),
        description: String::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (size, size),
            drill: through_hole.then(|| Nm::from_mm(1.0)),
            slot: None,
            layers: if through_hole {
                vec![Layer::TopCopper, Layer::BottomCopper]
            } else {
                vec![Layer::TopCopper]
            },
            mask_margin: None,
            rotation: Rotation::ZERO,
        }],
        bounds: Rect::from_center_size(Point::ORIGIN, (size, size)),
        courtyard: Rect::from_center_size(Point::ORIGIN, (size, size)),
        silk: Vec::new(),
    });
    let net = world.intern_net("SIG");
    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1", net));
    world.spawn_component(
        RefDes::new("J1"),
        Value::new("pin"),
        Position::from_mm(10.0, 10.0),
        Rotation(0),
        FootprintRef::new("PAD"),
        nets,
    );
    world.set_footprints(library.clone());
    (world, library)
}

fn flashes(through_hole: bool, layer: Layer) -> usize {
    let (mut world, library) = board(through_hole);
    let gerber = export_copper_layer(&mut world, &library, layer, &FORMAT).unwrap();
    gerber.lines().filter(|line| line.ends_with("D03*")).count()
}

/// The layers of an IPC-2581 document that hold a pad, by name.
fn ipc2581_pad_layers(through_hole: bool) -> Vec<String> {
    let (mut world, library) = board(through_hole);
    let (xml, _) = export_ipc2581(
        &mut world,
        &library,
        HouseTolerances::default(),
        "2026-09-25T00:00:00Z",
    );
    xml.split("<LayerFeature layerRef=\"")
        .skip(1)
        .filter(|section| {
            section
                .split("</LayerFeature>")
                .next()
                .is_some_and(|body| body.contains("<Set padUsage=\"TERMINATION\">"))
        })
        .map(|section| section.split('"').next().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn every_copper_file_flashes_a_through_hole_pad() {
    for (layer, _) in STACK {
        assert_eq!(flashes(true, layer), 1, "{layer:?}");
    }
    // The control: a top-side SMD pad is on the top file and nowhere else.
    for (layer, _) in STACK {
        let expected = usize::from(layer == Layer::TopCopper);
        assert_eq!(flashes(false, layer), expected, "SMD on {layer:?}");
    }
}

#[test]
fn every_ipc2581_copper_layer_carries_a_through_hole_pad() {
    let every: Vec<String> = STACK.iter().map(|(_, name)| name.to_string()).collect();
    assert_eq!(ipc2581_pad_layers(true), every);
    assert_eq!(ipc2581_pad_layers(false), vec!["F_Cu".to_string()]);
}
