//! A part on the back of the board is still on the back after KiCad.
//!
//! `cargo test -p cypcb-kicad --test a_bottom_part_survives_the_round_trip`
//!
//! `cypcb to-kicad` wrote every part as `(layer "F.Cu")`, with its reference
//! on `F.SilkS` and its value on `F.Fab`, whatever face the design put it on.
//! For a bottom part that produced a file KiCad reads as a **front-side part
//! whose pads are on the back** - and it went further wrong than that: the
//! footprint was written from the mirrored copy this project keeps internally,
//! under the derived name `0402@bottom`, so the pads were flipped a second
//! time by KiCad on load.
//!
//! The convention is not a guess. This project's own KiCad reader relies on
//! it, and its fixture states it: a back-side footprint in a real board file
//! carries `(layer "B.Cu")` and pad coordinates identical to a front-side one,
//! because KiCad mirrors the geometry itself from the layer.
//!
//! So the check here is the round trip: write a board with one part on each
//! face, read it back with this project's own parser, and require both parts
//! to come back where they started.

use cypcb_core::Nm;
use cypcb_kicad::board_writer::write_board;
use cypcb_kicad::pcb_parser::parse_kicad_pcb_str;
use cypcb_world::components::{
    FootprintRef, NetConnections, Position, RefDes, Rotation, Side, Value,
};
use cypcb_world::footprint::{bottom_name, mirrored_to_bottom, FootprintLibrary};
use cypcb_world::BoardWorld;

/// A board with R1 on the front and R2 on the back, the way sync builds one.
fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "sided".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        2,
    );

    // The world holds the flipped copy for a bottom part, exactly as
    // `sync_ast_to_world` registers it.
    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(mirrored_to_bottom(&base));
    world.set_footprints(library);

    let front = world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );
    world.ecs_mut().entity_mut(front).insert(Side::Top);

    let back = world.spawn_component(
        RefDes::new("R2"),
        Value::new("1k"),
        Position::from_mm(20.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new(bottom_name("0402")),
        NetConnections::new(),
    );
    world.ecs_mut().entity_mut(back).insert(Side::Bottom);

    world
}

fn written() -> String {
    write_board(&mut board(), "cypcb")
}

/// Which face each part comes back on, sorted by designator.
fn sides_after_round_trip(text: &str) -> Vec<(String, Side)> {
    let parsed = parse_kicad_pcb_str(text).expect("this project can read its own output");
    let mut world = parsed.world;
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&RefDes, &Side)>();
    let mut sides: Vec<(String, Side)> = query
        .iter(ecs)
        .map(|(refdes, side)| (refdes.as_str().to_string(), *side))
        .collect();
    sides.sort_by(|a, b| a.0.cmp(&b.0));
    sides
}

#[test]
fn each_part_comes_back_on_the_face_it_left_on() {
    assert_eq!(
        sides_after_round_trip(&written()),
        vec![
            ("R1".to_string(), Side::Top),
            ("R2".to_string(), Side::Bottom),
        ]
    );
}

#[test]
fn the_bottom_part_is_written_on_the_bottom_layer() {
    let text = written();
    let line = text
        .lines()
        .find(|line| line.contains("(footprint") && line.contains("20 10"))
        .unwrap_or_else(|| panic!("R2 is not in the file:\n{text}"));

    assert!(line.contains("(layer \"B.Cu\")"), "{line}");
}

#[test]
fn its_legend_is_printed_on_the_back() {
    // A back-side part with its reference on the front silkscreen is ink on
    // the wrong face of the board.
    let text = written();
    let r2 = text
        .lines()
        .find(|line| line.contains("reference \"R2\""))
        .unwrap_or_else(|| panic!("R2 has no reference:\n{text}"));

    assert!(r2.contains("B.SilkS"), "{r2}");
}

#[test]
fn the_footprint_keeps_the_name_the_design_asked_for() {
    // `0402@bottom` is this project's own arrangement for holding a mirrored
    // copy. A file written for somebody else must not carry it: it names a
    // library entry nobody has.
    let text = written();

    assert!(
        !text.contains("@bottom"),
        "the derived name leaked into the file:\n{text}"
    );
    assert_eq!(
        text.matches("\"cypcb:0402\"").count(),
        2,
        "both parts are 0402s:\n{text}"
    );
}

#[test]
fn the_geometry_is_not_mirrored_twice() {
    // KiCad mirrors a back-side footprint itself, from the layer. Writing the
    // mirrored copy as well puts pad 1 back where it started, so the file
    // describes a board that is not the one that was designed.
    //
    // Both parts are 0402s, so pad 1 sits at the same local x in both - which
    // is exactly what a real KiCad file looks like, and what this project's
    // own parser fixture states.
    let text = written();
    let pad_ones: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("(pad \"1\""))
        .collect();

    assert_eq!(pad_ones.len(), 2, "one per part:\n{text}");
    assert!(pad_ones[0].contains("(at -0.5 0)"), "{}", pad_ones[0]);
    assert!(pad_ones[1].contains("(at -0.5 0)"), "{}", pad_ones[1]);
    assert!(pad_ones[1].contains("\"B.Cu\""), "{}", pad_ones[1]);
}
