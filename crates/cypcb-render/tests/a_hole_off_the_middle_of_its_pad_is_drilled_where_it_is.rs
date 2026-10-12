//! A part fetched from EasyEDA with a hole off the middle of its pad is drilled
//! where the hole is, and its copper stays where the pad is.
//!
//! `cargo test -p cypcb-render --test a_hole_off_the_middle_of_its_pad_is_drilled_where_it_is`
//!
//! EasyEDA states a pad's hole as `holeCenter`, the twentieth field of its PAD
//! (the format document, read 2026-10-11). The viewer's parser read it for
//! nothing, so the engine knew a pad's hole only as the pad's own position, and
//! a drill file for a pad with its hole moved put the hole in the middle of the
//! land. The pads here are the viewer's output for the synthetic part in
//! `viewer/src/__tests__/fixtures/easyeda-offset-hole.json` - three 2.00 x
//! 3.00mm pads 2.54mm apart, the first two with their hole 0.5mm right and
//! 0.5mm down from their middle, the third with it in the middle - held by
//! `an-easyeda-hole-off-the-middle-of-its-pad-is-kept.test.ts` to be exactly
//! this file. One part stands square at (10,10) and one turned 30 degrees at
//! (30,10), so the offset has to turn with the part.

use cypcb_core::{Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::{export_excellon, DrillType};
use cypcb_export::gerber::export_copper_layer;
use cypcb_render::PadInfo;
use cypcb_world::footprint::{Footprint, FootprintLibrary};
use cypcb_world::{BoardWorld, Layer};

const SOURCE: &str = r#"version 1

board fetched {
    size 50mm x 20mm
    layers 2
}

component J1 connector "FETCHED" {
    at 10mm, 10mm
}

component J2 connector "FETCHED" {
    at 30mm, 10mm
    rotate 30
}
"#;

const FORMAT: CoordinateFormat = CoordinateFormat::FORMAT_MM_2_6;

fn pads() -> Vec<PadInfo> {
    let path = format!(
        "{}/tests/fixtures/easyeda-offset-hole-pads.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).expect("the engine reads what the viewer's parser wrote")
}

/// The board above, with `FETCHED` built from these pads.
fn board(pads: &[PadInfo]) -> (BoardWorld, FootprintLibrary) {
    let bounds = Rect::new(Point::from_mm(-6.0, -6.0), Point::from_mm(12.0, 6.0));
    let mut library = FootprintLibrary::new();
    library.register(
        Footprint {
            placement_origin: None,
            name: "FETCHED".to_string(),
            description: String::new(),
            pads: pads.iter().map(PadInfo::to_pad_def).collect(),
            bounds,
            courtyard: bounds,
            silk: Vec::new(),
        }
        .expect_pads_in_courtyard(),
    );
    let parsed = cypcb_parser::parse(SOURCE);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors.first());
    let mut world = BoardWorld::new();
    let synced = cypcb_world::sync_ast_to_world(&parsed.value, SOURCE, &mut world, &mut library);
    assert!(synced.errors.is_empty(), "{:?}", synced.errors.first());
    (world, library)
}

/// Every `X..Y..` hit of an Excellon file, in millimetres, sorted.
fn holes(file: &str) -> Vec<(f64, f64)> {
    let mut found: Vec<(f64, f64)> = file
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix('X')?;
            let (x, y) = rest.split_once('Y')?;
            Some((x.parse().ok()?, y.parse().ok()?))
        })
        .collect();
    found.sort_by(|a, b| a.partial_cmp(b).unwrap());
    found
}

fn drilled(pads: &[PadInfo]) -> Vec<(f64, f64)> {
    let (mut world, library) = board(pads);
    let file = export_excellon(
        &mut world,
        &library,
        &FORMAT,
        Some(DrillType::Plated),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .expect("the drill file is written");
    holes(&file)
}

/// Where a footprint point lands on a part at `at` turned `degrees`.
fn placed(at: (f64, f64), degrees: f64, point: (f64, f64)) -> (f64, f64) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    (
        at.0 + point.0 * cos - point.1 * sin,
        at.1 + point.0 * sin + point.1 * cos,
    )
}

fn assert_holes(got: &[(f64, f64)], want: &mut Vec<(f64, f64)>) {
    want.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(got.len(), want.len(), "{got:?} against {want:?}");
    for (g, w) in got.iter().zip(want.iter()) {
        assert!(
            (g.0 - w.0).abs() < 2e-6 && (g.1 - w.1).abs() < 2e-6,
            "drilled {g:?}, the hole is at {w:?}:\n{got:?}\n{want:?}"
        );
    }
}

/// The hole of each pad in the footprint's frame, in millimetres, from what
/// the part states: the pad's place and the 0.5mm right and 0.5mm down the first two move it.
fn holes_in_the_footprint() -> [(f64, f64); 3] {
    [(0.5, -0.5), (2.54 + 0.5, -0.5), (5.08, 0.0)]
}

#[test]
fn the_drill_goes_where_the_hole_is_on_a_square_part_and_a_turned_one() {
    let mut want: Vec<(f64, f64)> = Vec::new();
    for (at, degrees) in [((10.0, 10.0), 0.0), ((30.0, 10.0), 30.0)] {
        for hole in holes_in_the_footprint() {
            want.push(placed(at, degrees, hole));
        }
    }
    assert_holes(&drilled(&pads()), &mut want);
}

#[test]
fn control_without_the_offset_the_drill_goes_to_the_middle_of_the_pad() {
    // The same pads with what the viewer says about the hole taken away: every
    // hole is in the middle of its copper, which is where it was drilled
    // before the offset was read - 0.7mm from the hole on the first two.
    let mut bare = pads();
    for pad in &mut bare {
        pad.hole_offset_nm = None;
    }
    let mut want: Vec<(f64, f64)> = Vec::new();
    for (at, degrees) in [((10.0, 10.0), 0.0), ((30.0, 10.0), 30.0)] {
        for middle in [(0.0, 0.0), (2.54, 0.0), (5.08, 0.0)] {
            want.push(placed(at, degrees, middle));
        }
    }
    assert_holes(&drilled(&bare), &mut want);

    let with = drilled(&pads());
    let without = drilled(&bare);
    assert_ne!(with, without, "the offset changes what is drilled");
}

#[test]
fn the_copper_stays_on_the_pad_and_not_on_the_hole() {
    let (mut world, library) = board(&pads());
    let gerber = export_copper_layer(
        &mut world,
        &library,
        Layer::TopCopper,
        &FORMAT,
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .expect("the copper is written");
    let flashes: Vec<(f64, f64)> = gerber
        .lines()
        .filter(|line| line.ends_with("D03*"))
        .filter_map(|line| {
            let rest = line.strip_prefix('X')?;
            let (x, y) = rest.trim_end_matches("D03*").split_once('Y')?;
            Some((x.parse::<f64>().ok()? / 1e6, y.parse::<f64>().ok()? / 1e6))
        })
        .collect();
    let mut want: Vec<(f64, f64)> = Vec::new();
    for (at, degrees) in [((10.0, 10.0), 0.0), ((30.0, 10.0), 30.0)] {
        for middle in [(0.0, 0.0), (2.54, 0.0), (5.08, 0.0)] {
            want.push(placed(at, degrees, middle));
        }
    }
    let mut flashes = flashes;
    flashes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_holes(&flashes, &mut want);
}
