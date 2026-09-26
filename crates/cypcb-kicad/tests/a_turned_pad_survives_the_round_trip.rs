//! A turned pad comes back from KiCad turned the same way.
//!
//! `cargo test -p cypcb-kicad --test a_turned_pad_survives_the_round_trip`
//!
//! The writer put every pad down as `(at x y)`, with no angle. KiCad reads a
//! pad with no angle as axis aligned whatever its part's turn is, so every
//! pad of a turned part opened in KiCad lying the other way - an 0805 turned
//! 90 came back with its pads standing along the line between them. The
//! angle KiCad wants is the pad's turn on the board: the part's and its own
//! together.
//!
//! The control is a square pad on a square part, which writes no angle at
//! all, as KiCad itself writes it.

use cypcb_core::{Nm, Point, Rect};
use cypcb_kicad::board_writer::write_board;
use cypcb_kicad::pcb_parser::parse_kicad_pcb_str;
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Side, Value,
};
use cypcb_world::footprint::{
    base_name, bottom_name, mirrored_to_bottom, Footprint, FootprintLibrary, PadDef,
};
use cypcb_world::BoardWorld;

/// A footprint of one 1.0 by 2.0 pad at its origin, turned `turn` inside it.
fn footprint(name: &str, turn: Rotation) -> Footprint {
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.0), Nm::from_mm(2.0)));
    Footprint {
        name: name.to_string(),
        description: String::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.0), Nm::from_mm(2.0)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
            rotation: turn,
        }],
        bounds: body,
        courtyard: body,
        silk: Vec::new(),
    }
}

/// J1 square pad on a part turned 90, J2 pad turned 90 on a square part, J3
/// both turned 90, J4 both square, J5 a square pad on a part turned 90 on
/// the bottom.
fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(60.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(footprint("SQ", Rotation::ZERO));
    library.register(footprint("TU", Rotation::DEG_90));
    library.register_design(mirrored_to_bottom(&footprint("SQ", Rotation::ZERO)));
    world.set_footprints(library);
    for (i, (refdes, part, fp, side)) in [
        ("J1", Rotation::DEG_90, "SQ".to_string(), Side::Top),
        ("J2", Rotation::ZERO, "TU".to_string(), Side::Top),
        ("J3", Rotation::DEG_90, "TU".to_string(), Side::Top),
        ("J4", Rotation::ZERO, "SQ".to_string(), Side::Top),
        ("J5", Rotation::DEG_90, bottom_name("SQ"), Side::Bottom),
    ]
    .into_iter()
    .enumerate()
    {
        let entity = world.spawn_component(
            RefDes::new(refdes),
            Value::new("x"),
            Position::from_mm(10.0 * (i as f64 + 1.0), 10.0),
            part,
            FootprintRef::new(&fp),
            NetConnections::new(),
        );
        world.ecs_mut().entity_mut(entity).insert(side);
    }
    world
}

/// Each part's one pad as it lands on the board: its sides along the
/// board's x and y, by designator.
fn landed(world: &mut BoardWorld, library: &FootprintLibrary) -> Vec<(String, (Nm, Nm))> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&RefDes, &Rotation, &FootprintRef)>();
    let mut out: Vec<_> = query
        .iter(ecs)
        .map(|(refdes, rotation, fp)| {
            let footprint = library
                .get(fp.as_str())
                .or_else(|| library.get(base_name(fp.as_str())))
                .expect("the part's footprint is in the library");
            let pad = &footprint.pads[0];
            (
                refdes.as_str().to_string(),
                pad.outline(Point::ORIGIN, *rotation).size,
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn every_pad_comes_back_lying_the_way_it_left() {
    let mut world = board();
    let library = world.footprints().clone();
    let before = landed(&mut world, &library);

    let text = write_board(&mut world, "cypcb");
    let mut parsed = parse_kicad_pcb_str(&text).expect("this project reads its own output");
    let after = landed(&mut parsed.world, &parsed.library);

    assert_eq!(after, before, "{text}");
}

/// The `(at ...)` of J's one pad as written.
fn pad_at(text: &str, refdes: &str) -> String {
    let block = text
        .split("  (footprint ")
        .find(|block| block.contains(&format!("reference \"{refdes}\"")))
        .expect("the part is written");
    let pad = &block[block.find("(pad ").expect("the pad is written")..];
    pad[pad.find("(at ").unwrap()..]
        .split(')')
        .next()
        .unwrap()
        .to_string()
        + ")"
}

#[test]
fn the_angle_written_is_the_pad_s_turn_on_the_board() {
    let text = write_board(&mut board(), "cypcb");
    assert_eq!(pad_at(&text, "J1"), "(at 0 0 90)");
    assert_eq!(pad_at(&text, "J2"), "(at 0 0 90)");
    assert_eq!(pad_at(&text, "J3"), "(at 0 0 180)");
    assert_eq!(
        pad_at(&text, "J4"),
        "(at 0 0)",
        "a square pad on a square part says no angle"
    );
}
