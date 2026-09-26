//! The pick-and-place file places a through-hole part by the middle of its
//! pads and a surface-mount part by its origin.
//!
//! `cargo test -p cypcb-export --test a_placement_file_gives_the_part_middle`
//!
//! It wrote the footprint's origin for every part. A through-hole part is
//! anchored on pin 1 by convention, and the 1x12 header on `esp32_starter`
//! has its middle 13.97mm down from there: an assembler working from the
//! file puts the header most of its own length away from its holes. The
//! middle turns with the part the way a pad does. A surface-mount part keeps
//! its origin, which sits on the body, wherever its pads are - and a hole
//! with no copper, a locating peg, does not make it through-hole.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::cpl::export_cpl;
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// Pin 1 to the middle of the header, straight down in footprint terms.
const TO_MIDDLE_MM: f64 = 13.97;
const AT: (f64, f64) = (10.0, 40.0);

/// The header as `esp32_starter` defines it: origin on pin 1, eleven pins
/// below it at 2.54mm, courtyard around all twelve.
fn pin_header_1x12() -> Footprint {
    let pads = (0..12)
        .map(|i| PadDef {
            number: (i + 1).to_string(),
            shape: if i == 0 {
                PadShape::Rect
            } else {
                PadShape::Circle
            },
            position: Point::new(Nm::ZERO, Nm::from_mm(-2.54 * i as f64)),
            size: (Nm::from_mm(1.7), Nm::from_mm(1.7)),
            drill: Some(Nm::from_mm(1.0)),
            slot: None,
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        })
        .collect();
    Footprint {
        name: "PINHDR_1X12".into(),
        description: "Pin header 1x12, 2.54mm, vertical".into(),
        pads,
        bounds: Rect::default(),
        courtyard: Rect::from_center_size(
            Point::from_mm(0.0, -TO_MIDDLE_MM),
            (Nm::from_mm(3.54), Nm::from_mm(31.48)),
        ),
        silk: Vec::new(),
    }
}

/// A surface-mount module with its origin on the body and both pads 3mm
/// below it, the way a module's pads sit along one edge. `peg` adds a hole
/// with no copper under the body.
fn module_on_its_body(peg: bool) -> Footprint {
    let smd = |number: &str, x: f64| PadDef {
        number: number.into(),
        shape: PadShape::Rect,
        position: Point::from_mm(x, 3.0),
        size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
        drill: None,
        slot: None,
        layers: vec![Layer::TopCopper],
        mask_margin: None,
        rotation: Rotation::ZERO,
    };
    let mut pads = vec![smd("1", -2.0), smd("2", 2.0)];
    if peg {
        pads.push(PadDef {
            number: String::new(),
            shape: PadShape::Circle,
            position: Point::from_mm(0.0, -1.0),
            size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            drill: Some(Nm::from_mm(1.0)),
            slot: None,
            layers: Vec::new(),
            mask_margin: None,
            rotation: Rotation::ZERO,
        });
    }
    Footprint {
        name: "MODULE".into(),
        description: "Module with its pads along one edge".into(),
        pads,
        bounds: Rect::default(),
        courtyard: Rect::from_center_size(
            Point::from_mm(0.0, 3.0),
            (Nm::from_mm(5.5), Nm::from_mm(1.5)),
        ),
        silk: Vec::new(),
    }
}

/// Mid X and Mid Y of J1 with the header turned `degrees`.
fn written_at(degrees: f64) -> (f64, f64) {
    written_with(pin_header_1x12(), degrees)
}

/// Mid X and Mid Y of J1, drawn with `footprint` and turned `degrees`.
fn written_with(footprint: Footprint, degrees: f64) -> (f64, f64) {
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let name = footprint.name.clone();
    library.register_design(footprint);
    world.spawn_component(
        RefDes::new("J1"),
        Value::new("GPIO"),
        Position::from_mm(AT.0, AT.1),
        Rotation::from_degrees(degrees),
        FootprintRef::new(name),
        NetConnections::new(),
    );
    let csv = export_cpl(&mut world, &library, None).unwrap();
    let row = csv
        .lines()
        .find(|line| line.starts_with("J1,"))
        .unwrap_or_else(|| panic!("J1 is not in the file:\n{csv}"));
    let mm = |i: usize| -> f64 {
        row.split(',')
            .nth(i)
            .unwrap()
            .trim_end_matches("mm")
            .parse()
            .unwrap()
    };
    (mm(1), mm(2))
}

/// Where the middle lands, worked by hand: (0, -d) turned counter-clockwise
/// by `degrees` is (d sin, -d cos).
fn middle(degrees: f64) -> (f64, f64) {
    let r = degrees.to_radians();
    (AT.0 + TO_MIDDLE_MM * r.sin(), AT.1 - TO_MIDDLE_MM * r.cos())
}

fn assert_middle(degrees: f64) {
    let (x, y) = written_at(degrees);
    let (want_x, want_y) = middle(degrees);
    // The file carries three decimals.
    assert!(
        (x - want_x).abs() <= 0.0005 && (y - want_y).abs() <= 0.0005,
        "turned {degrees} degrees, the file places J1 at {x}, {y}; \
         its middle is at {want_x:.3}, {want_y:.3} and its pin 1 at {}, {}",
        AT.0,
        AT.1
    );
}

#[test]
fn a_header_anchored_on_pin_1_is_placed_by_its_middle() {
    assert_middle(0.0);
}

#[test]
fn the_middle_turns_with_the_part_a_quarter_turn() {
    assert_middle(90.0);
}

#[test]
fn the_middle_turns_with_the_part_at_45_degrees() {
    assert_middle(45.0);
}

fn assert_on_origin(peg: bool) {
    let (x, y) = written_with(module_on_its_body(peg), 90.0);
    assert!(
        (x - AT.0).abs() <= 0.0005 && (y - AT.1).abs() <= 0.0005,
        "the file places the module at {x}, {y}; its origin is at {}, {}",
        AT.0,
        AT.1
    );
}

#[test]
fn a_surface_mount_part_is_placed_by_its_origin() {
    assert_on_origin(false);
}

#[test]
fn a_hole_without_copper_leaves_a_surface_mount_part_on_its_origin() {
    assert_on_origin(true);
}
