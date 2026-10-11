//! A rounded rectangle pad stood at an angle keeps that angle in the files.
//!
//! `cargo test -p cypcb-export --test a_roundrect_stood_at_an_angle_flashes_a_turned_macro`
//!
//! `aperture_for_pad` gave a rounded rectangle the aperture of an unturned
//! one, so a part stood at, say, 30 degrees exported its rounded pads with
//! their sides along the board's axes. Measured 2026-10-11 on a board built
//! for it, a 2 by 1mm pad at 25% had 0.5243, 0.8324 and 1.2219 square
//! millimetres of its 1.9463 in the wrong place at 17.5, 30 and 45 degrees.
//! The sibling shapes were fixed by the angle-keeping apertures of the
//! rectangle and the oblong; this is the third.
//!
//! KiCad turns the pad's four corner centres in the macro's own parameters
//! and gives the macro no rotation (`AM_ROUND_RECT` in
//! `GERBER_PLOTTER::writeApertureList`, `common/plotters/GERBER_plotter.cpp`;
//! `include/plotters/gbr_plotter_aperture_macros.h`; KiCad master, read
//! 2026-10-11). Here the body rectangles take the rotation parameter, as the
//! turned rectangle does, and the corner circles are centred where the slope
//! puts them. What is held: the macro's text for the angles the owner's
//! chassis uses, quarter turns on the plain macro, the mask and the paste, and
//! - the check that matters - that the copper the file describes is the pad.

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

fn board(
    w: f64,
    h: f64,
    ratio: u8,
    own: Rotation,
    turn: Rotation,
) -> (BoardWorld, FootprintLibrary) {
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(w + 1.0), Nm::from_mm(h + 1.0)));
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "P".to_string(),
        description: String::new(),
        pads: vec![PadDef {
            hole_offset: None,
            number: "1".to_string(),
            shape: PadShape::RoundRect {
                corner_ratio: ratio,
            },
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
    });
    world.spawn_component(
        RefDes::new("U1"),
        Value::new("v"),
        Position::from_mm(10.0, 10.0),
        turn,
        FootprintRef::new("P"),
        NetConnections::new(),
    );
    world.set_footprints(library.clone());
    (world, library)
}

fn copper(w: f64, h: f64, ratio: u8, own: Rotation, turn: Rotation) -> String {
    let (mut world, library) = board(w, h, ratio, own, turn);
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
fn a_rounded_rectangle_at_30_degrees_is_a_turned_macro() {
    let file = copper(2.0, 1.0, 25, Rotation::ZERO, Rotation::from_degrees(30.0));
    assert!(
        file.contains("%ADD10TQ10*%\n"),
        "the pad flashes a turned macro, not the plain one:\n{file}"
    );
    // Radius 0.25mm; the corner centres (0.75, 0.25) turned 30 degrees.
    assert_eq!(
        macro_body(&file, "TQ10"),
        "%AMTQ10*\n21,1,2.000000,0.500000,0,0,30.000*\n\
         21,1,1.500000,1.000000,0,0,30.000*\n\
         1,1,0.500000,0.524519,0.591506,0*\n\
         1,1,0.500000,-0.774519,-0.158494,0*\n\
         1,1,0.500000,0.774519,0.158494,0*\n\
         1,1,0.500000,-0.524519,-0.591506,0*",
        "{file}"
    );
}

#[test]
fn the_angle_from_the_chassis_range_is_stated_whole() {
    let file = copper(2.0, 1.0, 25, Rotation::ZERO, Rotation::from_degrees(17.5));
    assert_eq!(
        macro_body(&file, "TQ10"),
        "%AMTQ10*\n21,1,2.000000,0.500000,0,0,17.500*\n\
         21,1,1.500000,1.000000,0,0,17.500*\n\
         1,1,0.500000,0.640111,0.463959,0*\n\
         1,1,0.500000,-0.790464,0.012900,0*\n\
         1,1,0.500000,0.790464,-0.012900,0*\n\
         1,1,0.500000,-0.640111,-0.463959,0*",
        "{file}"
    );
}

#[test]
fn a_turn_past_a_quarter_turn_keeps_the_slope_of_what_is_left() {
    // 135 degrees stands the pad the way 45 does, with its sides swapped: the
    // 2 by 1 pad is 1 by 2 along the board's axes, then turned 45.
    let file = copper(2.0, 1.0, 25, Rotation::ZERO, Rotation::from_degrees(135.0));
    let body = macro_body(&file, "TQ10");
    assert!(
        body.contains("21,1,1.000000,1.500000,0,0,45.000*\n"),
        "{body}"
    );
    assert!(
        body.contains("21,1,0.500000,2.000000,0,0,45.000*\n"),
        "{body}"
    );
    // The corner centres of the swapped pad, (0.25, 0.75), turned 45.
    assert!(
        body.contains("1,1,0.500000,-0.353553,0.707107,0*"),
        "{body}"
    );
}

#[test]
fn the_corner_ratio_comes_with_it() {
    // 1 by 2mm at 40% has a 0.4mm radius; centres (0.1, 0.6) turned 17.5.
    let file = copper(1.0, 2.0, 40, Rotation::ZERO, Rotation::from_degrees(17.5));
    let body = macro_body(&file, "TQ10");
    assert!(
        body.contains("21,1,1.000000,1.200000,0,0,17.500*\n"),
        "{body}"
    );
    assert!(
        body.contains("1,1,0.800000,-0.085052,0.602301,0*"),
        "{body}"
    );
}

#[test]
fn the_pad_s_own_turn_and_the_part_s_add_up() {
    let file = copper(
        2.0,
        1.0,
        25,
        Rotation::from_degrees(20.0),
        Rotation::from_degrees(10.0),
    );
    assert!(
        macro_body(&file, "TQ10").contains("21,1,2.000000,0.500000,0,0,30.000*\n"),
        "{file}"
    );
}

#[test]
fn a_quarter_turn_keeps_the_plain_macro() {
    for degrees in [0.0, 90.0, 180.0, 270.0] {
        let file = copper(
            2.0,
            1.0,
            25,
            Rotation::ZERO,
            Rotation::from_degrees(degrees),
        );
        assert!(file.contains("%ADD10RR10*%\n"), "{degrees}:\n{file}");
        assert!(!file.contains("TQ"), "{degrees}:\n{file}");
    }
}

/// Whether the macro's primitives cover the point (millimetres, in the flash's
/// own frame). Reads the `21` centre-line rectangles with their rotation and
/// the `1` circles.
fn covered(body: &str, x: f64, y: f64) -> bool {
    body.lines().skip(1).any(|line| {
        let f: Vec<f64> = line
            .trim_end_matches('*')
            .split(',')
            .map(|v| v.parse().expect("a number"))
            .collect();
        match f[0] as i32 {
            21 => {
                // exposure, width, height, cx, cy, rotation (centre at origin)
                let (s, c) = (-f[6]).to_radians().sin_cos();
                let (u, v) = (x * c - y * s, x * s + y * c);
                (u - f[4]).abs() <= f[2] / 2.0 && (v - f[5]).abs() <= f[3] / 2.0
            }
            1 => {
                let (dx, dy) = (x - f[3], y - f[4]);
                dx * dx + dy * dy <= (f[2] / 2.0) * (f[2] / 2.0)
            }
            other => panic!("primitive {other} is not read here"),
        }
    })
}

/// Whether the pad the design states covers the point: its size and corner
/// radius, turned by `degrees`.
fn pad_covers(w: f64, h: f64, ratio: f64, degrees: f64, x: f64, y: f64) -> bool {
    let r = w.min(h) * ratio / 100.0;
    let (s, c) = (-degrees).to_radians().sin_cos();
    let (u, v) = (x * c - y * s, x * s + y * c);
    let (dx, dy) = (
        (u.abs() - (w / 2.0 - r)).max(0.0),
        (v.abs() - (h / 2.0 - r)).max(0.0),
    );
    dx * dx + dy * dy <= r * r
}

/// Square millimetres where the flash and the pad disagree, on a 10 micron
/// grid. The grid's own noise on a boundary this long is far below a
/// thousandth of a square millimetre.
fn disagreement(body: &str, w: f64, h: f64, ratio: f64, degrees: f64) -> f64 {
    let step = 0.01;
    let n = (3.2 / step) as i32;
    let mut cells = 0u32;
    for i in 0..n {
        for j in 0..n {
            let (x, y) = (
                -1.6 + (i as f64 + 0.5) * step,
                -1.6 + (j as f64 + 0.5) * step,
            );
            if covered(body, x, y) != pad_covers(w, h, ratio, degrees, x, y) {
                cells += 1;
            }
        }
    }
    f64::from(cells) * step * step
}

#[test]
fn the_copper_the_file_describes_is_the_pad() {
    for (w, h, ratio, degrees) in [
        (2.0, 1.0, 25, 17.5),
        (2.0, 1.0, 25, 30.0),
        (2.0, 1.0, 25, 45.0),
        (2.0, 1.0, 25, 135.0),
        (1.0, 2.0, 40, 17.5),
        (1.0, 2.0, 40, 30.0),
        (1.0, 2.0, 40, 45.0),
    ] {
        let file = copper(w, h, ratio, Rotation::ZERO, Rotation::from_degrees(degrees));
        let name = if file.contains("%AMTQ10*") {
            "TQ10"
        } else {
            "RR10"
        };
        let wrong = disagreement(&macro_body(&file, name), w, h, f64::from(ratio), degrees);
        assert!(
            wrong < 0.002,
            "{w}x{h} at {ratio}%, {degrees} degrees: {wrong} mm2 of the flash is not the pad"
        );
    }
}

#[test]
fn control_the_unturned_macro_disagrees_with_a_turned_pad() {
    // The macro the pad was flashed with before: a 2 by 1 pad at 30 degrees
    // drawn square. The same check must find a large part of it in the wrong
    // place, or it would pass whatever the file said.
    let file = copper(2.0, 1.0, 25, Rotation::ZERO, Rotation::ZERO);
    let wrong = disagreement(&macro_body(&file, "RR10"), 2.0, 1.0, 25.0, 30.0);
    assert!(
        wrong > 0.7,
        "the unturned macro is {wrong} mm2 off a pad at 30 degrees"
    );
}

fn openings(turn: Rotation) -> (String, String) {
    let (mut world, library) = board(2.0, 1.0, 25, Rotation::ZERO, turn);
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
    // The board's 0.05 expansion each side: 2.1 by 1.1, radius 25% of 1.1.
    let body = macro_body(&mask, "TQ10");
    assert!(
        body.contains("21,1,2.100000,0.550000,0,0,30.000*\n"),
        "{mask}"
    );
    assert!(
        body.contains("21,1,1.550000,1.100000,0,0,30.000*\n"),
        "{mask}"
    );
    assert!(body.contains("1,1,0.550000,0.533670,0.625657,0*"), "{mask}");
}

#[test]
fn the_paste_prints_the_turned_pad_as_it_is() {
    let (_, paste) = openings(Rotation::from_degrees(30.0));
    let body = macro_body(&paste, "TQ10");
    assert!(
        body.contains("21,1,2.000000,0.500000,0,0,30.000*\n"),
        "{paste}"
    );
    assert!(
        body.contains("1,1,0.500000,0.524519,0.591506,0*"),
        "{paste}"
    );
}

#[test]
fn the_paste_shrinks_the_turned_pad_and_keeps_its_angle() {
    let (mut world, library) = board(2.0, 1.0, 25, Rotation::ZERO, Rotation::from_degrees(30.0));
    let config = MaskPasteConfig {
        paste_reduction: 0.1,
        ..MaskPasteConfig::default()
    };
    let paste = export_solderpaste(
        &mut world,
        &library,
        Side::Top,
        &FORMAT,
        &config,
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .unwrap();
    // 90% of 2.0 by 1.0, radius 25% of 0.9.
    assert!(
        macro_body(&paste, "TQ10").contains("21,1,1.800000,0.450000,0,0,30.000*\n"),
        "{paste}"
    );
}
