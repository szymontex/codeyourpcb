//! A fabrication file has to agree with its own header.
//!
//! `cargo test -p cypcb-export --test every_number_agrees_with_the_header_above_it`
//!
//! Every Gerber this project ever wrote declared `%FSLAX26Y26*%` - two integer
//! digits and six decimal ones, with the decimal point implied, which is what
//! the `26` means - and then wrote `X3.730000`. The header said the point
//! would not be there and the data put one in. A reader that believes the
//! declaration and a reader that believes the data get answers a thousand
//! times apart.
//!
//! Nothing caught it because every test read the coordinates the way the
//! exporter wrote them. Ten of them called `parse::<f64>()` on the digits and
//! took millimetres out, which is only correct if you already assume the bug.
//! A test that decodes a file the way its header says to is a different test
//! from one that decodes it the way the writer happened to encode it, and only
//! the first can fail.
//!
//! The rule is not "Gerber has no decimal point". It is per-number:
//!
//! - **Gerber coordinate data** (`X...Y...D0n`) - no point. `%FS` declared it.
//! - **Gerber aperture definitions** (`%ADD10C,1.500000*%`) - a point. This is
//!   a size in the file's units and nothing declared a scale for it; without
//!   the point it is a 1.5-metre aperture.
//! - **Excellon** - a point. Its header here is `METRIC,TZ`, which names no
//!   digit count at all, so an integer would have no declared scale and a
//!   reader assuming the usual 3.3 puts the hole at 3730mm.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::export_excellon;
use cypcb_export::gerber::copper::export_copper_layer;
use cypcb_export::gerber::outline::export_outline;
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position, RefDes,
    Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A two-pin header: drilled, so copper, apertures and drill are all exercised
/// by one fixture. Built rather than parsed - this crate does not depend on the
/// parser, and the question here is about what the writer emits.
fn two_pin_header() -> Footprint {
    let pad = |number: &str, x: f64| PadDef {
        number: number.to_string(),
        shape: PadShape::Circle,
        position: Point::from_mm(x, 0.0),
        size: (Nm::from_mm(1.7), Nm::from_mm(1.7)),
        drill: Some(Nm::from_mm(1.0)),
        layers: vec![Layer::TopCopper, Layer::BottomCopper],
    };

    Footprint {
        name: "PinHeader_1x02".to_string(),
        description: "two drilled pads".to_string(),
        pads: vec![pad("1", 0.0), pad("2", 2.54)],
        bounds: Rect::from_center_size(
            Point::from_mm(1.27, 0.0),
            (Nm::from_mm(4.24), Nm::from_mm(1.7)),
        ),
        courtyard: Rect::from_center_size(
            Point::from_mm(1.27, 0.0),
            (Nm::from_mm(4.74), Nm::from_mm(2.2)),
        ),
        silk: Vec::new(),
    }
}

fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "headers".to_string(),
        (Nm::from_mm(40.0), Nm::from_mm(30.0)),
        2,
    );

    let mut library = FootprintLibrary::new();
    library.register(two_pin_header());

    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1".to_string(), NetId::new(1)));
    nets.add(PinConnection::new("2".to_string(), NetId::new(2)));
    world.spawn_component(
        RefDes::new("J1"),
        Value::new("conn"),
        Position::from_mm(10.0, 15.0),
        Rotation(0),
        FootprintRef::new("PinHeader_1x02"),
        nets,
    );

    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

/// The `X.../Y...` payload of a Gerber coordinate line, if it is one.
fn coordinate_payload(line: &str) -> Option<&str> {
    if !line.starts_with('X') && !line.starts_with('Y') {
        return None;
    }
    // D01 draws, D02 moves, D03 flashes. Everything else on an X line is not
    // coordinate data.
    if !line.contains("D01") && !line.contains("D02") && !line.contains("D03") {
        return None;
    }
    Some(line)
}

#[test]
fn no_gerber_coordinate_carries_a_decimal_point() {
    let (mut world, library) = board();
    let format = CoordinateFormat::FORMAT_MM_2_6;

    let files = [
        (
            "copper",
            export_copper_layer(&mut world, &library, Layer::TopCopper, &format)
                .expect("top copper"),
        ),
        ("outline", export_outline(&world, &format).expect("outline")),
    ];

    for (what, gerber) in &files {
        let declaration = gerber
            .lines()
            .find(|line| line.starts_with("%FS"))
            .unwrap_or_else(|| panic!("{what} states no coordinate format"));
        assert!(
            declaration.contains("X26"),
            "{what} declares {declaration}, and this test only knows 2.6"
        );

        let offenders: Vec<&str> = gerber
            .lines()
            .filter_map(coordinate_payload)
            .filter(|line| line.contains('.'))
            .collect();
        assert!(
            offenders.is_empty(),
            "{what} declares {declaration} - six implied decimals, no point - \
             and then writes one:\n{}",
            offenders.join("\n")
        );

        // The other half of the same question: a coordinate has to be there at
        // all. A file with no `X...D0n` lines would pass the check above by
        // saying nothing.
        assert!(
            gerber.lines().filter_map(coordinate_payload).count() > 0,
            "{what} carries no coordinates at all"
        );
    }
}

#[test]
fn an_aperture_definition_keeps_its_decimal_point() {
    let (mut world, library) = board();
    let format = CoordinateFormat::FORMAT_MM_2_6;
    let gerber =
        export_copper_layer(&mut world, &library, Layer::TopCopper, &format).expect("top copper");

    let apertures: Vec<&str> = gerber
        .lines()
        .filter(|line| line.starts_with("%ADD"))
        .collect();
    assert!(
        !apertures.is_empty(),
        "a layer with pads defines apertures:\n{gerber}"
    );

    for aperture in &apertures {
        assert!(
            aperture.contains('.'),
            "an aperture size is a number in the file's units and nothing \
             declared a scale for it: {aperture} would be a metre across"
        );
    }

    // And the size is the pad's, not a thousand times it. The pad is 1.7mm.
    let sizes: Vec<f64> = apertures
        .iter()
        .filter_map(|line| line.split(',').nth(1))
        .filter_map(|rest| rest.split(['X', '*']).next())
        .filter_map(|value| value.parse::<f64>().ok())
        .collect();
    assert!(
        sizes.iter().any(|s| (s - 1.7).abs() < 0.01),
        "no aperture is the 1.7mm pad's size: {sizes:?}"
    );
}

#[test]
fn a_drill_file_keeps_its_decimal_point() {
    let (mut world, library) = board();
    let format = CoordinateFormat::FORMAT_MM_2_6;
    let drill = export_excellon(&mut world, &library, &format, None).expect("drill file");

    assert!(
        drill.contains("METRIC"),
        "the drill file states its units:\n{drill}"
    );

    let holes: Vec<&str> = drill.lines().filter(|line| line.starts_with('X')).collect();
    assert!(!holes.is_empty(), "a board with drilled pads has hits");

    for hole in &holes {
        assert!(
            hole.contains('.'),
            "`METRIC,TZ` names no digit count, so an integer here has no \
             declared scale and a reader assuming 3.3 puts this hole a metre \
             away: {hole}"
        );
    }

    // J1 sits at 10mm, 15mm, so its first hole does too. Read as millimetres
    // straight off the file, which is what the decimal point is for.
    let first = holes[0];
    let x: f64 = first[1..first.find('Y').expect("a hit has both axes")]
        .parse()
        .expect("a readable x");
    assert!(
        (x - 10.0).abs() < 0.01,
        "the first hole reads as {x}mm, and J1 pad 1 is at 10mm: {first}"
    );

    // The tool table is a size, not a coordinate: same rule as an aperture.
    let tools: Vec<&str> = drill
        .lines()
        .filter(|line| line.starts_with('T') && line.contains('C'))
        .collect();
    assert!(!tools.is_empty(), "the drill file lists its tools");
    for tool in &tools {
        assert!(
            tool.contains('.'),
            "a drill diameter without its point is a metre wide: {tool}"
        );
    }
}
