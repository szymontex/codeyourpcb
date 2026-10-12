//! The SVG, DXF and PDF plots draw in the order the board was built.
//!
//! `cargo test -p cypcb-export --test a_plot_comes_in_the_order_the_board_was_built`
//!
//! A bevy query walks its archetypes in the order of a hash map whose seed is
//! drawn once per process, so rows of one kind that sit in more than one
//! archetype came out in a different order from one run of the program to the
//! next, and the plots written from them differed byte for byte. The writers
//! now ask through `in_build_order`, which sorts by entity. Which archetype is
//! met first is a coin the process throws, so a test that only compares two
//! runs passes half the time with the fault in. This one does not rely on the
//! coin: every other row of each kind carries one more component than its
//! neighbours, so the rows sit in two archetypes, and neither order the two can
//! be walked in is the order the rows were made in. A writer that walks a plain
//! query draws them in the wrong order every time.

use cypcb_core::{Nm, Point, Rect};

use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::components::{
    BoardDimension, BoardText, FootprintRef, Layer, LcscPart, NetConnections, PadShape,
    PinConnection, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

fn land() -> Footprint {
    let size = Nm::from_mm(1.6);
    Footprint {
        placement_origin: None,
        name: "LAND".to_string(),
        description: "one round through-hole land".to_string(),
        pads: vec![PadDef {
            hole_offset: None,
            number: "1".to_string(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (size, size),
            drill: Some(Nm::from_mm(0.8)),
            slot: None,
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        }],
        bounds: Rect::from_center_size(Point::ORIGIN, (size, size)),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.4), Nm::from_mm(2.4))),
        silk: Vec::new(),
    }
}

/// What tells one row of a kind from the next in the text of a plot: a
/// coordinate no other row and no other kind shares, in millimetres as the
/// plots write them.
const PARTS: [f64; 4] = [11.111, 12.222, 13.333, 14.444];
const TRACKS: [f64; 4] = [16.171, 17.272, 18.373, 19.474];
const VIAS: [f64; 4] = [25.151, 25.252, 25.353, 25.454];
const DIMENSIONS: [f64; 4] = [30.161, 31.262, 32.363, 33.464];
const TEXTS: [f64; 4] = [5.5, 8.5, 11.5, 14.5];

/// A coordinate as the SVG and the DXF write it: millimetres.
fn mm(value: f64) -> String {
    format!("{value:.3}")
}

/// A coordinate as the PDF writes it: points, 72 to the inch.
fn pt(value: f64) -> String {
    format!("{:.3}", value * 72.0 / 25.4)
}

/// Four rows of each kind, made in the order of the arrays above. When
/// `split` is set, every second one has a part number attached - which nothing
/// in a plot or a file reads - so that the rows of each kind sit in two
/// archetypes.
fn board(split: bool) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "order".to_string(),
        (Nm::from_mm(40.0), Nm::from_mm(30.0)),
        2,
    );
    let mut library = FootprintLibrary::new();
    library.register(land());
    world.set_footprints(library.clone());
    let net = world.intern_net("A");
    let mut apart = Vec::new();

    for (i, x) in PARTS.iter().enumerate() {
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", net));
        let part = world.spawn_component(
            RefDes::new(format!("R{}", i + 1)),
            Value::new("x"),
            Position::from_mm(*x, 5.0),
            Rotation::ZERO,
            FootprintRef::new("LAND"),
            nets,
        );
        apart.push((i, part));
    }
    for (i, y) in TRACKS.iter().enumerate() {
        let track = world.spawn_entity((
            Trace {
                segments: vec![TraceSegment::new(
                    Point::from_mm(2.0, *y),
                    Point::from_mm(8.0, *y),
                )],
                width: Nm::from_mm(0.2),
                layer: Layer::TopCopper,
                net_id: net,
                locked: false,
                source: TraceSource::Manual,
            },
            net,
        ));
        apart.push((i, track));
    }
    for (i, x) in VIAS.iter().enumerate() {
        let via = world.spawn_entity(Via::new(Point::from_mm(*x, 20.0), net));
        apart.push((i, via));
    }
    for (i, x) in DIMENSIONS.iter().enumerate() {
        let dimension = world.spawn_entity(BoardDimension {
            from: Point::from_mm(*x, 1.0),
            to: Point::from_mm(*x + 2.0, 1.0),
            offset: BoardDimension::DEFAULT_OFFSET,
        });
        apart.push((i, dimension));
    }
    for (i, x) in TEXTS.iter().enumerate() {
        let text = world.spawn_entity(BoardText {
            content: "T".to_string(),
            position: Point::from_mm(*x, 26.0),
            layer: Layer::TopSilk,
            height: BoardText::DEFAULT_HEIGHT,
        });
        apart.push((i, text));
    }
    for (i, entity) in apart {
        if split && i % 2 == 1 {
            world
                .ecs_mut()
                .entity_mut(entity)
                .insert(LcscPart("C0".to_string()));
        }
    }
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

type Plot = fn(&mut BoardWorld, &FootprintLibrary, Layer) -> String;

/// How a plot writes a coordinate.
type Write = fn(f64) -> String;

/// Each plot with the way it writes a coordinate.
fn plots() -> [(&'static str, Plot, Write); 3] {
    [
        ("svg", cypcb_export::svg::plot_layer, mm),
        ("dxf", cypcb_export::dxf::plot_layer, mm),
        ("pdf", cypcb_export::pdf::plot_layer, pt),
    ]
}

/// Where each coordinate first appears in the text, or `None`.
fn places(text: &str, values: &[f64], write: fn(f64) -> String) -> Vec<Option<usize>> {
    values.iter().map(|v| text.find(&write(*v))).collect()
}

fn in_order(places: &[Option<usize>]) -> bool {
    places.iter().all(Option::is_some) && places.windows(2).all(|w| w[0] < w[1])
}

#[test]
fn each_kind_is_drawn_in_the_order_it_was_made() {
    let kinds: [(&str, &[f64; 4]); 4] = [
        ("parts", &PARTS),
        ("tracks", &TRACKS),
        ("vias", &VIAS),
        ("dimensions", &DIMENSIONS),
    ];
    let (mut world, library) = board(true);
    for (name, plot, write) in plots() {
        let text = plot(&mut world, &library, Layer::TopCopper);
        for (kind, values) in kinds {
            let found = places(&text, values.as_slice(), write);
            assert!(
                in_order(&found),
                "{name}: the {kind} are not drawn in the order they were made: {found:?}"
            );
        }
    }
}

#[test]
fn control_the_check_reads_a_wrong_order_as_wrong() {
    // The same board with the rows' markers read in the reverse order is out of
    // order, and a marker that is not in the text is not in order either.
    let (mut world, library) = board(true);
    let text = cypcb_export::svg::plot_layer(&mut world, &library, Layer::TopCopper);
    let mut backwards = PARTS;
    backwards.reverse();
    assert!(
        !in_order(&places(&text, &backwards, mm)),
        "reversed is not in order"
    );
    assert!(
        !in_order(&places(&text, &[99.999, 11.111], mm)),
        "a missing marker is not in order"
    );
}

#[test]
fn a_writer_that_walks_a_plain_query_is_caught_by_this_board() {
    // The rows of each kind really are in two archetypes, and a plain walk
    // really is out of order, in whichever order the two are walked: this is
    // what the board is built to do, held here so the board cannot drift into
    // one that every writer passes.
    let (mut world, _) = board(true);
    let mut walked: Vec<(i64, i64)> = Vec::new();
    {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        for via in query.iter(ecs) {
            walked.push((via.position.x.0, via.position.y.0));
        }
    }
    let made: Vec<i64> = VIAS.iter().map(|x| Point::from_mm(*x, 20.0).x.0).collect();
    let got: Vec<i64> = walked.iter().map(|(x, _)| *x).collect();
    assert_ne!(
        got, made,
        "a plain query walks the vias out of the order they were made"
    );
}

/// Every file the board is written to, as text, for one board.
fn files(world: &mut BoardWorld, library: &FootprintLibrary) -> Vec<(&'static str, String)> {
    use cypcb_export::coords::CoordinateFormat;
    use cypcb_export::excellon::{export_excellon, DrillType};
    use cypcb_export::gerber::{export_copper_layer, export_silkscreen, Side, SilkConfig};
    use cypcb_export::ipc2581::{export_ipc2581, HouseTolerances};
    use cypcb_export::stamp::Stamp;

    let format = CoordinateFormat::FORMAT_MM_2_6;
    let mut files = Vec::new();
    for (name, plot, _) in plots() {
        for layer in [Layer::TopCopper, Layer::BottomCopper] {
            files.push((name, plot(world, library, layer)));
        }
    }
    files.push((
        "gerber copper",
        export_copper_layer(world, library, Layer::TopCopper, &format, Stamp::UNIX_EPOCH).unwrap(),
    ));
    files.push((
        "excellon",
        export_excellon(
            world,
            library,
            &format,
            Some(DrillType::Plated),
            Stamp::UNIX_EPOCH,
        )
        .unwrap(),
    ));
    files.push((
        "gerber silk",
        export_silkscreen(
            world,
            library,
            Side::Top,
            &format,
            &SilkConfig::default(),
            Stamp::UNIX_EPOCH,
        )
        .unwrap(),
    ));
    files.push((
        "ipc-2581",
        export_ipc2581(
            world,
            library,
            HouseTolerances::default(),
            Stamp::UNIX_EPOCH,
        )
        .0,
    ));
    files.push((
        "ipc-d-356",
        cypcb_export::ipc356::export_ipc356(world, library, "t").0,
    ));
    files
}

#[test]
fn no_file_depends_on_what_else_a_row_carries() {
    // The same four rows of each kind, once in one archetype and once in two:
    // what a row carries beside what a file reads is not a thing a file says.
    let (mut one, library) = board(false);
    let (mut two, _) = board(true);
    let (one, two) = (files(&mut one, &library), files(&mut two, &library));
    assert_eq!(one.len(), two.len());
    for ((name, plain), (_, split)) in one.iter().zip(&two) {
        assert!(!plain.is_empty(), "{name} is a file");
        assert_eq!(
            plain, split,
            "{name} reads the rows in another order when they sit in two archetypes"
        );
    }
}
