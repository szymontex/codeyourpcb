//! Every plot and document of the copper draws the teardrops the board holds.
//!
//! `cargo test -p cypcb-export --test every_plot_draws_the_teardrop_copper`
//!
//! The Gerber writer has written the fillets since the teardrop became copper
//! in the board; the SVG, DXF, PDF and IPC-2581 writers drew none, so a board
//! looked at, handed to a mechanical tool or sent as IPC-2581 showed copper
//! that was not the copper in the Gerber. The board here is one round land
//! with a track leaving it and a second land at the other end, so there are two
//! fillets, both on the top layer. Each writer is held to drawing both there
//! and none on the bottom, and a board that asks for none gets none.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::ipc2581::{export_ipc2581, HouseTolerances};
use cypcb_export::stamp::Stamp;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, PinConnection, Position, RefDes, Rotation,
    Teardrops, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::teardrop::teardrop_copper;
use cypcb_world::BoardWorld;

fn land() -> Footprint {
    let size = Nm::from_mm(1.6);
    Footprint {
        placement_origin: None,
        name: "LAND".to_string(),
        description: "one round land".to_string(),
        pads: vec![PadDef {
            hole_offset: None,
            number: "1".to_string(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (size, size),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        }],
        bounds: Rect::from_center_size(Point::ORIGIN, (size, size)),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.4), Nm::from_mm(2.4))),
        silk: Vec::new(),
    }
}

fn board(asked: bool) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(land());
    world.set_footprints(library.clone());
    let net = world.intern_net("A");
    for (refdes, x) in [("R1", 10.0), ("R2", 18.0)] {
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", net));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("x"),
            Position::from_mm(x, 10.0),
            Rotation::ZERO,
            FootprintRef::new("LAND"),
            nets,
        );
    }
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(10.0, 10.0),
                Point::from_mm(18.0, 10.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: net,
            locked: false,
            source: TraceSource::Manual,
        },
        net,
    ));
    if asked {
        assert!(world.set_teardrops(Teardrops::default()));
    }
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

fn mm(nm: i64) -> String {
    format!("{:.3}", nm as f64 / 1_000_000.0)
}

/// The corners of every teardrop the board holds, in millimetres as the plots
/// write them.
fn corners(world: &mut BoardWorld) -> Vec<Vec<(String, String)>> {
    teardrop_copper(world)
        .into_iter()
        .map(|(_, copper, _)| {
            copper
                .outline
                .iter()
                .map(|p| (mm(p.x.0), mm(p.y.0)))
                .collect()
        })
        .collect()
}

#[test]
fn the_svg_draws_each_teardrop_as_a_polygon() {
    let (mut world, library) = board(true);
    let held = corners(&mut world);
    assert_eq!(held.len(), 2, "the board holds both fillets");
    let top = cypcb_export::svg::plot_layer(&mut world, &library, Layer::TopCopper);
    assert_eq!(top.matches("<polygon ").count(), 2, "{top}");
    for corners in &held {
        let points = corners
            .iter()
            .map(|(x, y)| format!("{x},{y}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            top.contains(&format!("points=\"{points}\"")),
            "{points} in {top}"
        );
    }
    let bottom = cypcb_export::svg::plot_layer(&mut world, &library, Layer::BottomCopper);
    assert_eq!(bottom.matches("<polygon ").count(), 0);

    let (mut plain, library) = board(false);
    let top = cypcb_export::svg::plot_layer(&mut plain, &library, Layer::TopCopper);
    assert_eq!(
        top.matches("<polygon ").count(),
        0,
        "control: none asked, none drawn"
    );
}

#[test]
fn the_dxf_draws_each_teardrop_as_a_closed_polyline() {
    let (mut world, library) = board(true);
    let held = corners(&mut world);
    let top = cypcb_export::dxf::plot_layer(&mut world, &library, Layer::TopCopper);
    // On the copper layer a closed polyline is flagged 70 / 1; a track is an
    // open one, 70 / 0, and the board edge is closed but on its own layer.
    let closed = |text: &str| text.matches("POLYLINE\n8\nF_Cu\n66\n1\n70\n1\n").count();
    assert_eq!(
        closed(&top),
        2,
        "two closed polylines on the copper layer: {top}"
    );
    for corners in &held {
        for (x, y) in corners {
            assert!(
                top.contains(&format!("10\n{x}\n20\n{y}\n")),
                "vertex {x},{y} in {top}"
            );
        }
    }
    let bottom = cypcb_export::dxf::plot_layer(&mut world, &library, Layer::BottomCopper);
    assert!(!bottom.contains("POLYLINE\n8\nB_Cu\n66\n1\n70\n1\n"));

    let (mut plain, library) = board(false);
    let top = cypcb_export::dxf::plot_layer(&mut plain, &library, Layer::TopCopper);
    assert_eq!(closed(&top), 0, "control: none asked, none drawn");
}

#[test]
fn the_pdf_fills_each_teardrop() {
    let (mut world, library) = board(true);
    let top = cypcb_export::pdf::plot_layer(&mut world, &library, Layer::TopCopper);
    assert_eq!(top.matches("h\nf\n").count(), 2, "{top}");
    let bottom = cypcb_export::pdf::plot_layer(&mut world, &library, Layer::BottomCopper);
    assert_eq!(bottom.matches("h\nf\n").count(), 0);

    let (mut plain, library) = board(false);
    let top = cypcb_export::pdf::plot_layer(&mut plain, &library, Layer::TopCopper);
    assert_eq!(
        top.matches("h\nf\n").count(),
        0,
        "control: none asked, none drawn"
    );
}

#[test]
fn the_ipc2581_document_states_each_teardrop_as_a_polygon_of_its_net() {
    let polygons = |world: &mut BoardWorld, library: &FootprintLibrary| {
        export_ipc2581(
            world,
            library,
            HouseTolerances::default(),
            Stamp::UNIX_EPOCH,
        )
        .0
    };
    let (mut world, library) = board(true);
    let held = corners(&mut world);
    let document = polygons(&mut world, &library);
    // The board's outline is a polygon too, in its own section: only the copper
    // layer's features are counted.
    let top_features = |document: &str| -> String {
        let from = document
            .find("<LayerFeature layerRef=\"F_Cu\">")
            .expect("the top copper has features");
        let rest = &document[from..];
        let to = rest.find("</LayerFeature>").expect("the section ends");
        rest[..to].to_string()
    };
    let features = top_features(&document);
    assert_eq!(features.matches("<Polygon>").count(), 2, "{features}");
    for corners in &held {
        let (x, y) = &corners[0];
        assert!(
            document.contains(&format!("<PolyBegin x=\"{x}\" y=\"{y}\"/>")),
            "{x},{y} in {document}"
        );
    }
    // One set for the track and one for each fillet, all of net A.
    assert_eq!(
        features.matches("<Set net=\"A\">").count(),
        3,
        "the polygons are sets of their net: {features}"
    );

    let (mut plain, library) = board(false);
    let document = polygons(&mut plain, &library);
    assert_eq!(
        top_features(&document).matches("<Polygon>").count(),
        0,
        "control: none asked, none drawn"
    );
}
