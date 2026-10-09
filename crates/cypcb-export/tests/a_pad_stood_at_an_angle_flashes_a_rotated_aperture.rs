//! A pad turned by an angle that is not a multiple of 90 degrees keeps that
//! angle in the files, as an aperture macro.
//!
//! `cargo test -p cypcb-export --test a_pad_stood_at_an_angle_flashes_a_rotated_aperture`
//!
//! The aperture a rectangle or an oblong pad was flashed with stated a width
//! and a height and nothing about an angle, so a part stood at, say, 30
//! degrees exported its copper standing square: measured 2026-10-07, a 2 by
//! 1mm rectangle at 30 degrees had 0.9282 of its 2 square millimetres in the
//! wrong place, and a 2.4 by 1mm oblong at 45 degrees 1.6892 of its 1.823 -
//! the first number grows past half the pad's own copper. This repository's
//! example boards only turn parts by multiples of 90, which is why no test
//! saw it; the chassis the owner is laying out turns them 17.5 to 135.
//!
//! The Gerber specification's own rotating-rectangle example passes the angle
//! into a center-line macro primitive (revision 2026.05, section 4.5.1, the
//! `Box` macro; read 2026-10-07), and KiCad flashes the same pads the same
//! way: a `RotRect` macro for a turned rectangle, a `HorizOval` whose cap
//! centres carry the angle for a turned oblong (`FlashPadRect` and
//! `FlashPadOval` in `common/plotters/GERBER_plotter.cpp`,
//! `include/plotters/gbr_plotter_aperture_macros.h`; read 2026-10-07). A
//! stadium gets its angle in its geometry rather than as a rotation
//! parameter for the same reason KiCad's authors give there: readers break
//! on shapes whose whole rotation they have to apply.

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

/// One pad, so the first aperture in a file is the one under test.
fn one_pad_footprint(name: &str, shape: PadShape, w: f64, h: f64, own: Rotation) -> Footprint {
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(w + 1.0), Nm::from_mm(h + 1.0)));
    Footprint {
        name: name.to_string(),
        description: String::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape,
            position: Point::ORIGIN,
            size: (Nm::from_mm(w), Nm::from_mm(h)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper, Layer::TopMask, Layer::TopPaste],
            mask_margin: None,
            rotation: own,
        }],
        bounds: body,
        courtyard: body,
        silk: Vec::new(),
    }
}

/// A board with one part on it, registered and placed.
fn one_part_board(shape: PadShape, w: f64, h: f64, own: Rotation, turn: Rotation) -> String {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(one_pad_footprint("P", shape, w, h, own));
    world.spawn_component(
        RefDes::new("U1"),
        Value::new("v"),
        Position::from_mm(10.0, 10.0),
        turn,
        FootprintRef::new("P"),
        NetConnections::new(),
    );
    world.set_footprints(library.clone());
    export_copper_layer(
        &mut world,
        &library,
        Layer::TopCopper,
        &FORMAT,
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .unwrap()
}

/// The body of the macro a D-code flashes with: every line between its `%AM`
/// and its closing `%`.
fn macro_body(gerber: &str, name: &str) -> String {
    let start = gerber
        .find(&format!("%AM{name}*"))
        .unwrap_or_else(|| panic!("no macro {name} in {gerber}"));
    let rest = &gerber[start..];
    let end = rest.find("\n%\n").expect("a macro closes with %");
    rest[..end].to_string()
}

#[test]
fn a_rectangle_at_30_degrees_is_a_rotating_center_line_macro() {
    let file = one_part_board(
        PadShape::Rect,
        2.0,
        1.0,
        Rotation::ZERO,
        Rotation::from_degrees(30.0),
    );
    assert!(
        file.contains("%ADD10TR10*%\n"),
        "the pad flashes a macro aperture, not a plain R:\n{file}"
    );
    assert_eq!(
        macro_body(&file, "TR10"),
        "%AMTR10*\n21,1,2.000000,1.000000,0,0,30.000*",
        "{file}"
    );
}

#[test]
fn the_angle_from_the_chassis_range_is_stated_whole() {
    // 17.5 degrees, the shallowest turn the owner's chassis places.
    let file = one_part_board(
        PadShape::Rect,
        2.0,
        1.0,
        Rotation::ZERO,
        Rotation::from_degrees(17.5),
    );
    assert!(
        file.contains("21,1,2.000000,1.000000,0,0,17.500*\n"),
        "{file}"
    );
}

#[test]
fn a_turn_past_a_quarter_turn_keeps_the_slope_of_what_is_left() {
    // 135 degrees stands the pad the way 45 does, with its sides swapped.
    let file = one_part_board(
        PadShape::Rect,
        2.0,
        1.0,
        Rotation::ZERO,
        Rotation::from_degrees(135.0),
    );
    assert!(
        file.contains("21,1,1.000000,2.000000,0,0,45.000*\n"),
        "{file}"
    );
}

#[test]
fn the_pad_s_own_turn_and_the_part_s_add_up() {
    // A pad turned 20 in its footprint on a part turned 10 stands at 30.
    let file = one_part_board(
        PadShape::Rect,
        2.0,
        1.0,
        Rotation::from_degrees(20.0),
        Rotation::from_degrees(10.0),
    );
    assert!(
        file.contains("21,1,2.000000,1.000000,0,0,30.000*\n"),
        "{file}"
    );
}

#[test]
fn an_oblong_carries_its_angle_in_its_cap_centres() {
    // 2.4 by 1.0 at 30 degrees: caps 0.7mm out along the long axis, which is
    // 0.606218 across and 0.35 up, with a line the width of the short side.
    let file = one_part_board(
        PadShape::Oblong,
        2.4,
        1.0,
        Rotation::ZERO,
        Rotation::from_degrees(30.0),
    );
    assert!(
        file.contains("%ADD10TO10*%\n"),
        "the oblong flashes a macro aperture, not a plain O:\n{file}"
    );
    assert_eq!(
        macro_body(&file, "TO10"),
        "%AMTO10*\n20,1,1.000000,0.606218,0.350000,-0.606218,-0.350000,0*\n\
         1,1,1.000000,0.606218,0.350000,0*\n\
         1,1,1.000000,-0.606218,-0.350000,0*",
        "{file}"
    );
}

#[test]
fn an_oblong_taller_than_wide_turns_its_long_axis_a_quarter_further() {
    // The chassis turns a part 17.5; its vertical oblong's long axis runs
    // at 107.5, whose caps are 0.210494 across and 0.667602 up.
    let file = one_part_board(
        PadShape::Oblong,
        1.0,
        2.4,
        Rotation::ZERO,
        Rotation::from_degrees(17.5),
    );
    assert_eq!(
        macro_body(&file, "TO10"),
        "%AMTO10*\n20,1,1.000000,-0.210494,0.667602,0.210494,-0.667602,0*\n\
         1,1,1.000000,-0.210494,0.667602,0*\n\
         1,1,1.000000,0.210494,-0.667602,0*",
        "{file}"
    );
}

#[test]
fn a_circle_has_no_angle_to_lose() {
    let file = one_part_board(
        PadShape::Circle,
        1.6,
        1.6,
        Rotation::ZERO,
        Rotation::from_degrees(30.0),
    );
    assert!(
        file.contains("%ADD10C,1.600000*%\n"),
        "a circle stays a plain aperture:\n{file}"
    );
}

/// The mask opening and the paste print around a pad stood at an angle.
fn openings(turn: Rotation) -> (String, String) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(one_pad_footprint(
        "P",
        PadShape::Rect,
        2.0,
        1.0,
        Rotation::ZERO,
    ));
    world.spawn_component(
        RefDes::new("U1"),
        Value::new("v"),
        Position::from_mm(10.0, 10.0),
        turn,
        FootprintRef::new("P"),
        NetConnections::new(),
    );
    world.set_footprints(library.clone());
    let mask = export_soldermask(
        &mut world,
        &library,
        Side::Top,
        &FORMAT,
        &MaskPasteConfig::default(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .unwrap();
    let paste = export_solderpaste(
        &mut world,
        &library,
        Side::Top,
        &FORMAT,
        &MaskPasteConfig::default(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .unwrap();
    (mask, paste)
}

#[test]
fn the_mask_opens_wider_around_the_turned_pad_at_its_own_angle() {
    let (mask, _) = openings(Rotation::from_degrees(30.0));
    // The board's 0.05 expansion each side, on both sides of the pad.
    assert!(
        mask.contains("21,1,2.100000,1.100000,0,0,30.000*\n"),
        "{mask}"
    );
}

#[test]
fn the_paste_prints_the_turned_pad_as_it_is() {
    let (_, paste) = openings(Rotation::from_degrees(30.0));
    assert!(
        paste.contains("21,1,2.000000,1.000000,0,0,30.000*\n"),
        "{paste}"
    );
}
