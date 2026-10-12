//! A slot whose pad states it off the middle of its copper is milled where the
//! slot is, from the same middle the round holes use.
//!
//! `cargo test -p cypcb-export --test a_slot_off_the_middle_of_its_pad_is_milled_where_it_is`
//!
//! The drill file writes a slot as the path of the bit's centre, `X..Y..G85X..
//! Y..`: the two ends of the travel, a bit's radius short of the slot's own
//! ends, along the slot's long axis. Those two points are the hole's middle
//! plus and minus half the travel, and the hole's middle is the pad's plus its
//! [`PadDef::hole_offset`] - the offset in the pad's own frame, turned with the
//! pad and with its part. The part here is turned 30 degrees and the pad 90 in
//! its footprint, so a writer that applies the offset in the wrong frame, or
//! only to one end, lands somewhere else.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::{export_excellon, DrillType};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

const FORMAT: CoordinateFormat = CoordinateFormat::FORMAT_MM_2_6;

fn drill_file(offset: Option<Point>) -> String {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let mut library = FootprintLibrary::new();
    let body = Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(8.0), Nm::from_mm(8.0)));
    library.register(Footprint {
        name: "S".to_string(),
        description: String::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Oblong,
            position: Point::from_mm(1.0, 2.0),
            size: (Nm::from_mm(3.0), Nm::from_mm(1.6)),
            drill: Some(Nm::from_mm(1.0)),
            slot: Some((Nm::from_mm(2.4), Nm::from_mm(1.0))),
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
            mask_margin: None,
            rotation: Rotation::DEG_90,
            hole_offset: offset,
        }],
        bounds: body,
        courtyard: body,
        silk: Vec::new(),
    });
    world.set_footprints(library.clone());
    world.spawn_component(
        RefDes::new("J1"),
        Value::new("v"),
        Position::from_mm(20.0, 15.0),
        Rotation::from_degrees(30.0),
        FootprintRef::new("S"),
        NetConnections::new(),
    );
    export_excellon(
        &mut world,
        &library,
        &FORMAT,
        Some(DrillType::Plated),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .expect("the drill file is written")
}

/// The slot's two ends, in millimetres.
fn ends(file: &str) -> ((f64, f64), (f64, f64)) {
    let line = file
        .lines()
        .find(|line| line.contains("G85"))
        .unwrap_or_else(|| panic!("no slot in\n{file}"));
    let (start, end) = line.split_once("G85").unwrap();
    let pair = |text: &str| {
        let (x, y) = text.trim_start_matches('X').split_once('Y').unwrap();
        (x.parse::<f64>().unwrap(), y.parse::<f64>().unwrap())
    };
    (pair(start), pair(end))
}

fn turn(point: (f64, f64), degrees: f64) -> (f64, f64) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    (point.0 * cos - point.1 * sin, point.0 * sin + point.1 * cos)
}

/// The slot's ends on the board for a hole `offset` (millimetres, in the pad's
/// frame) from the pad's middle, worked out here from the geometry alone.
fn expected(offset: (f64, f64)) -> ((f64, f64), (f64, f64)) {
    // In the footprint: the pad at (1, 2), its offset turned by the pad's own
    // 90 degrees, and the travel (2.4 - 1.0) / 2 = 0.7mm along the pad's x,
    // turned by the same 90.
    let turned_offset = turn(offset, 90.0);
    let middle = (1.0 + turned_offset.0, 2.0 + turned_offset.1);
    let half = turn((0.7, 0.0), 90.0);
    let place = |p: (f64, f64)| {
        let t = turn(p, 30.0);
        (20.0 + t.0, 15.0 + t.1)
    };
    (
        place((middle.0 - half.0, middle.1 - half.1)),
        place((middle.0 + half.0, middle.1 + half.1)),
    )
}

fn same(got: ((f64, f64), (f64, f64)), want: ((f64, f64), (f64, f64))) -> bool {
    let close = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 2e-6 && (a.1 - b.1).abs() < 2e-6;
    close(got.0, want.0) && close(got.1, want.1)
}

#[test]
fn both_ends_of_the_slot_move_with_the_hole() {
    let file = drill_file(Some(Point::from_mm(0.3, -0.2)));
    let want = expected((0.3, -0.2));
    assert!(
        same(ends(&file), want),
        "{:?} against {want:?}",
        ends(&file)
    );
}

#[test]
fn control_a_slot_with_no_offset_is_milled_from_the_pads_middle() {
    let file = drill_file(None);
    let want = expected((0.0, 0.0));
    assert!(
        same(ends(&file), want),
        "{:?} against {want:?}",
        ends(&file)
    );
    // And it is not where the offset puts it.
    assert!(!same(ends(&file), expected((0.3, -0.2))));
}
