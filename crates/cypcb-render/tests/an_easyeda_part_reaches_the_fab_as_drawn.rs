//! A part fetched from EasyEDA reaches the fab files as its drawing gives it.
//!
//! `cargo test -p cypcb-render --test an_easyeda_part_reaches_the_fab_as_drawn`
//!
//! The viewer parses the part and hands the engine pads and a legend. Both
//! sides are held to the same files in `tests/fixtures/`: the viewer's test
//! `an-easyeda-part-arrives-turned-slotted-and-drilled.test.ts` checks that its
//! parser writes exactly them, and these tests take them to the drill file and
//! the legend Gerber. Until 2026-09-26 the engine named an arc's angles
//! `start_angle`/`end_angle` in degrees and the viewer sent
//! `startAngle`/`endAngle` in radians: serde skipped the one and defaulted the
//! other, and every arc became a full circle on the board.

use cypcb_core::{Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::{export_excellon, DrillType};
use cypcb_export::gerber::silk::export_silkscreen;
use cypcb_export::gerber::Side;
use cypcb_render::{PadInfo, PcbEngine, SilkInfo};
use cypcb_world::footprint::{Footprint, FootprintLibrary};
use cypcb_world::BoardWorld;

const SOURCE: &str = r#"version 1

board fetched {
    size 30mm x 20mm
    layers 2
}

component J1 connector "FETCHED" {
    at 10mm, 10mm
}
"#;

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The board above, with `FETCHED` built from the viewer's pads and legend.
fn board(pads: &[PadInfo], silk: &[SilkInfo]) -> (BoardWorld, FootprintLibrary) {
    let bounds = Rect::new(Point::from_mm(-6.0, -6.0), Point::from_mm(6.0, 6.0));
    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "FETCHED".to_string(),
        description: String::new(),
        pads: pads.iter().map(PadInfo::to_pad_def).collect(),
        bounds,
        courtyard: bounds,
        silk: silk.iter().flat_map(SilkInfo::to_shapes).collect(),
    });
    let parsed = cypcb_parser::parse(SOURCE);
    let mut world = BoardWorld::new();
    cypcb_world::sync_ast_to_world(&parsed.value, SOURCE, &mut world, &mut library);
    (world, library)
}

/// The tool diameters an Excellon file's header declares, in millimetres.
///
/// EasyEDA writes four decimals of 0.254mm, so a 0.65mm hole arrives as
/// 0.649986mm: a drawing's figure is compared to the nearest micrometre.
fn tools_mm(file: &str) -> Vec<f64> {
    file.lines()
        .filter(|line| line.starts_with('T'))
        .filter_map(|line| line.split_once('C'))
        .filter_map(|(_, diameter)| diameter.parse::<f64>().ok())
        .map(|mm| (mm * 1000.0).round() / 1000.0)
        .collect()
}

#[test]
fn a_quarter_arc_from_the_viewer_is_a_quarter_arc_in_the_legend() {
    let silk_json = fixture("easyeda-quarter-arc-silk.json");
    let silk: Vec<SilkInfo> =
        serde_json::from_str(&silk_json).expect("the engine reads the viewer's arc");

    // Through the engine the way the viewer sends it: a quarter turn at 32
    // segments to the circle is eight.
    let mut engine = PcbEngine::new();
    assert_eq!(engine.register_footprint("FETCHED", "[]", &silk_json), "");
    assert_eq!(
        silk[0].to_shapes().len(),
        8,
        "a quarter arc is eight segments, not 32"
    );

    // Radius 2.54mm about the part's origin at 10,10: from the right of it to
    // above it, and never round to the left.
    let (mut world, library) = board(&[], &silk);
    let gerber = export_silkscreen(
        &mut world,
        &library,
        Side::Top,
        &CoordinateFormat::FORMAT_MM_2_6,
        &Default::default(),
    )
    .expect("the legend exports");
    assert!(
        gerber.contains("X12540000Y10000000D02*"),
        "starts at 0 degrees:\n{gerber}"
    );
    assert!(
        gerber.contains("X10000000Y12540000D01*"),
        "ends at 90 degrees:\n{gerber}"
    );
    assert!(
        !gerber.contains("X7460000Y10000000"),
        "a quarter arc never reaches 180 degrees:\n{gerber}"
    );
}

#[test]
fn a_legend_field_the_engine_does_not_name_is_refused() {
    // The names the engine used before 2026-09-26. Skipping them is how an
    // arc became a circle; now they are an error at the boundary.
    let renamed = r#"[{"type":"arc","cx":0,"cy":0,"radius":1000000,"width":150000,
        "start_angle":0.0,"end_angle":90.0}]"#;
    let refused = PcbEngine::new().register_footprint("FETCHED", "[]", renamed);
    assert!(refused.contains("unknown field"), "{refused}");
}

#[test]
fn the_pegs_are_drilled_unplated_and_the_shell_slots_milled() {
    let pads: Vec<PadInfo> = serde_json::from_str(&fixture("easyeda-usb4105-pads.json"))
        .expect("the engine reads the viewer's pads");
    let (mut world, library) = board(&pads, &[]);
    let format = CoordinateFormat::FORMAT_MM_2_6;

    // GCT USB4105, drawing B4: 2x Ø0.65 non-plated, 5.78 apart.
    let npth = export_excellon(&mut world, &library, &format, Some(DrillType::NonPlated))
        .expect("the non-plated drills export");
    assert_eq!(tools_mm(&npth), vec![0.65], "a 0.65mm tool:\n{npth}");
    assert_eq!(
        npth.matches("X7.10").count() + npth.matches("X12.89").count(),
        2,
        "{npth}"
    );
    assert!(
        !npth.contains("G85"),
        "a peg is drilled, not milled:\n{npth}"
    );

    // Four shell slots, 0.60 wide: milled, with the 0.60 tool.
    let pth = export_excellon(&mut world, &library, &format, Some(DrillType::Plated))
        .expect("the plated drills export");
    assert_eq!(pth.matches("G85").count(), 4, "four slots milled:\n{pth}");

    // Along the pads' long side, which is Y: the bit travels the slot's
    // length less its width, 1.70 - 0.60 and 1.40 - 0.60.
    let mut travels: Vec<(f64, f64)> = pth
        .lines()
        .filter_map(|line| line.split_once("G85"))
        .map(|(from, to)| {
            let xy = |s: &str| {
                let (x, y) = s.trim_start_matches('X').split_once('Y').expect("X..Y..");
                (x.parse::<f64>().unwrap(), y.parse::<f64>().unwrap())
            };
            let ((x0, y0), (x1, y1)) = (xy(from), xy(to));
            (
                ((x1 - x0).abs() * 100.0).round() / 100.0,
                ((y1 - y0).abs() * 100.0).round() / 100.0,
            )
        })
        .collect();
    travels.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(
        travels,
        vec![(0.0, 0.8), (0.0, 0.8), (0.0, 1.1), (0.0, 1.1)],
        "{pth}"
    );
    assert_eq!(
        tools_mm(&pth),
        vec![0.6],
        "one 0.60mm tool, and no peg:\n{pth}"
    );
}
