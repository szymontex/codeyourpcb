//! A part's legend goes where the part is, in the file and on the screen.
//!
//! `cargo test -p cypcb-render --features native --test the_legend_goes_where_the_part_is`
//!
//! The board model keeps one legend per footprint and prints it on the face
//! the part is on: the top Gerber for a part on the top, the bottom Gerber for
//! one on the bottom, mirrored as its pads are. The snapshot hands the browser
//! that same legend with the part's `side`, and the browser decides the face
//! from the side. What is held here is that the two say the same thing, and
//! that nothing else is in the way: a legend shape names no side of its own,
//! so the field the viewer once sent for it is refused at the boundary.
//!
//! The 3D view read that field until 2026-10-12, and the engine never sent
//! it, so every legend was built under the board. The viewer's test
//! `a-legend-is-built-on-the-face-its-part-is-on.test.ts` holds the same two
//! parts to the figures found in the files here.

use cypcb_core::{Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::silk::export_silkscreen;
use cypcb_export::gerber::Side;
use cypcb_render::{PcbEngine, SilkInfo};
use cypcb_world::footprint::{Footprint, FootprintLibrary};
use cypcb_world::BoardWorld;

/// R1 on the top at 10mm, R2 under the board at 30mm: one footprint.
fn source(second: &str) -> String {
    format!(
        r#"version 1

board sided {{
    size 40mm x 20mm
    layers 2
}}

component R1 resistor "FETCHED" {{
    at 10mm, 10mm
}}

component R2 resistor "FETCHED" {{
    at 30mm, 10mm
{second}
}}
"#
    )
}

/// One line, 1mm to 2mm right of the origin, in the viewer's own words.
const LINE: &str = r#"[{"type":"segment","x1":1000000,"y1":0,"x2":2000000,"y2":0,"width":150000}]"#;

const PAD: &str = r#"[{"number":"1","x_nm":0,"y_nm":0,"width_nm":300000,
                       "height_nm":300000,"shape":"rect","layer_mask":1,"drill_nm":null}]"#;

/// The legend file of one face, for the board above.
fn gerber(source: &str, side: Side) -> String {
    let silk: Vec<SilkInfo> =
        serde_json::from_str(LINE).expect("the engine reads the viewer's line");
    let bounds = Rect::new(Point::from_mm(-6.0, -6.0), Point::from_mm(6.0, 6.0));
    let mut library = FootprintLibrary::new();
    library.register(
        Footprint {
            placement_origin: None,
            name: "FETCHED".to_string(),
            description: String::new(),
            pads: vec![],
            bounds,
            courtyard: bounds,
            silk: silk.iter().flat_map(SilkInfo::to_shapes).collect(),
        }
        .expect_pads_in_courtyard(),
    );
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "the board does not parse");
    let mut world = BoardWorld::new();
    let synced = cypcb_world::sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(synced.errors.is_empty(), "the board does not load");
    export_silkscreen(
        &mut world,
        &library,
        side,
        &CoordinateFormat::FORMAT_MM_2_6,
        &Default::default(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .expect("the legend exports")
}

/// The snapshot the browser gets for the same board.
fn snapshot(source: &str) -> serde_json::Value {
    let mut engine = PcbEngine::new();
    assert_eq!(engine.register_footprint("FETCHED", PAD, LINE), "");
    assert_eq!(engine.load_source(source), "");
    serde_json::from_str(&engine.get_snapshot()).expect("the snapshot is JSON")
}

/// The two ends of every line of a part's legend, as the Gerber writes a point,
/// placed where the snapshot puts the part.
fn ends(part: &serde_json::Value) -> Vec<String> {
    let (x, y) = (
        part["x_nm"].as_i64().unwrap(),
        part["y_nm"].as_i64().unwrap(),
    );
    assert_eq!(
        part["rotation_mdeg"].as_i64(),
        Some(0),
        "the test parts are not turned"
    );
    let silk = part["silk"].as_array().expect("the part has a legend");
    assert!(!silk.is_empty(), "{} has no legend", part["refdes"]);
    let mut points = Vec::new();
    for shape in silk {
        for (px, py) in [("x1", "y1"), ("x2", "y2")] {
            let (dx, dy) = (shape[px].as_i64().unwrap(), shape[py].as_i64().unwrap());
            points.push(format!("X{}Y{}D0", x + dx, y + dy));
        }
    }
    points
}

fn face(part: &serde_json::Value) -> Side {
    match part["side"].as_str() {
        Some("bottom") => Side::Bottom,
        _ => Side::Top,
    }
}

fn other(side: Side) -> Side {
    match side {
        Side::Top => Side::Bottom,
        Side::Bottom => Side::Top,
    }
}

#[test]
fn each_legend_is_in_the_file_of_the_face_its_part_is_on() {
    let source = source("    side bottom");
    let shot = snapshot(&source);
    let files = [
        (Side::Top, gerber(&source, Side::Top)),
        (Side::Bottom, gerber(&source, Side::Bottom)),
    ];
    let file = |side: Side| &files.iter().find(|(s, _)| *s == side).unwrap().1;

    let parts = shot["components"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    let mut seen = [false; 2];
    for part in parts {
        let side = face(part);
        seen[matches!(side, Side::Bottom) as usize] = true;
        for end in ends(part) {
            assert!(
                file(side).contains(&end),
                "{} {end} is not in its own file",
                part["refdes"]
            );
            assert!(
                !file(other(side)).contains(&end),
                "{} {end} is in the other file",
                part["refdes"]
            );
        }
    }
    assert_eq!(seen, [true, true], "one part on each face");
}

#[test]
fn the_bottom_legend_is_the_top_one_turned_over() {
    // 1mm to 2mm right of R1 at 10mm is 11 to 12. Under the board the same
    // line is 1mm to 2mm left of R2 at 30mm: 29 to 28.
    let source = source("    side bottom");
    let shot = snapshot(&source);
    let parts = shot["components"].as_array().unwrap();
    let r1 = parts.iter().find(|p| p["refdes"] == "R1").unwrap();
    let r2 = parts.iter().find(|p| p["refdes"] == "R2").unwrap();
    assert_eq!(ends(r1), ["X11000000Y10000000D0", "X12000000Y10000000D0"]);
    assert_eq!(ends(r2), ["X29000000Y10000000D0", "X28000000Y10000000D0"]);
}

#[test]
fn control_two_parts_on_the_top_put_both_legends_in_the_top_file() {
    // The same board with R2 left alone: the check above sees a difference
    // only because a part was turned over.
    let source = source("");
    let shot = snapshot(&source);
    let (top, bottom) = (gerber(&source, Side::Top), gerber(&source, Side::Bottom));
    for part in shot["components"].as_array().unwrap() {
        assert_eq!(face(part), Side::Top);
        for end in ends(part) {
            assert!(
                top.contains(&end),
                "{} {end} is not in the top file",
                part["refdes"]
            );
            assert!(
                !bottom.contains(&end),
                "{} {end} is in the bottom file",
                part["refdes"]
            );
        }
    }
}

#[test]
fn a_legend_shape_in_the_snapshot_names_no_side() {
    let shot = snapshot(&source("    side bottom"));
    let mut shapes = 0;
    for part in shot["components"].as_array().unwrap() {
        for shape in part["silk"].as_array().unwrap() {
            assert!(
                shape.get("layer").is_none(),
                "{} sends a layer: {shape}",
                part["refdes"]
            );
            shapes += 1;
        }
    }
    assert_eq!(shapes, 2, "one line on each part");
}

#[test]
fn a_layer_on_a_legend_shape_is_refused() {
    // What the viewer sent until 2026-10-12. The engine had no reader for it.
    let with_layer =
        r#"[{"type":"segment","x1":0,"y1":0,"x2":1000000,"y2":0,"width":150000,"layer":"bottom"}]"#;
    let refused = PcbEngine::new().register_footprint("FETCHED", PAD, with_layer);
    assert!(refused.contains("unknown field `layer`"), "{refused}");

    // Control: the same line without it is taken.
    let without = r#"[{"type":"segment","x1":0,"y1":0,"x2":1000000,"y2":0,"width":150000}]"#;
    assert_eq!(
        PcbEngine::new().register_footprint("FETCHED", PAD, without),
        ""
    );
}

#[test]
fn the_legend_of_a_real_part_is_taken_as_the_viewer_writes_it() {
    // The viewer's parser writes this file for the AP2112K (its own test holds
    // it to the supplier's drawing): four lines and the pin-1 arc, no layer.
    let path = format!(
        "{}/tests/fixtures/easyeda-ap2112k-silk.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let json = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let silk: Vec<SilkInfo> =
        serde_json::from_str(&json).expect("the engine reads the viewer's legend");
    assert_eq!(silk.len(), 5, "four lines and an arc");

    let mut engine = PcbEngine::new();
    assert_eq!(engine.register_footprint("SOT25", PAD, &json), "");
    let source = r#"version 1

board b {
    size 20mm x 20mm
    layers 2
}

component U1 ic "SOT25" {
    at 10mm, 10mm
    side bottom
}
"#;
    assert_eq!(engine.load_source(source), "");
    let shot: serde_json::Value = serde_json::from_str(&engine.get_snapshot()).unwrap();
    let part = &shot["components"][0];
    assert_eq!(part["side"], "bottom");
    let wanted: usize = silk.iter().map(|s| s.to_shapes().len()).sum();
    assert_eq!(
        part["silk"].as_array().unwrap().len(),
        wanted,
        "the arc is segments, nothing is lost"
    );
}
