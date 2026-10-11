//! The screen shows the teardrops the fabricator is sent.
//!
//! `cargo test -p cypcb-render --test the_snapshot_carries_the_teardrops`
//!
//! A teardrop is copper in the board, and the Gerber writer, the checker and
//! the plots read it there. The snapshot the viewer draws from carried none, so
//! the screen showed a board without the fillet its files hold. Each teardrop
//! is a polygon in the snapshot - its corners, its layer, its net - and the
//! corners are the corners the Gerber region has, in the same order.

use cypcb_render::PcbEngine;

const SOURCE: &str = r#"
board fillets {
    size 30mm x 20mm
    layers 2
    teardrops
}

footprint LAND {
    description "one round land"
    courtyard 2.4mm x 2.4mm

    pad 1 circle at 0mm, 0mm size 1.6mm x 1.6mm
}

component R1 resistor "LAND" {
    value "a"
    at 10mm, 10mm
}

component R2 resistor "LAND" {
    value "b"
    at 18mm, 10mm
}

net A {
    R1.1
    R2.1
}

trace A {
    from R1.1
    to R2.1
    layer Top
    width 0.2mm
}
"#;

fn snapshot_of(source: &str) -> serde_json::Value {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(source);
    assert!(errors.is_empty(), "the source did not load: {errors}");
    serde_json::from_str(&engine.get_snapshot()).expect("the snapshot is JSON")
}

/// Every region of a Gerber file, as its corners in nanometres, the closing
/// point left off.
fn regions(gerber: &str) -> Vec<Vec<[i64; 2]>> {
    let mut found = Vec::new();
    let mut current: Option<Vec<[i64; 2]>> = None;
    for line in gerber.lines() {
        if line == "G36*" {
            current = Some(Vec::new());
        } else if line == "G37*" {
            if let Some(mut corners) = current.take() {
                corners.pop();
                found.push(corners);
            }
        } else if let (Some(corners), Some(rest)) = (current.as_mut(), line.strip_prefix('X')) {
            let (x, rest) = rest.split_once('Y').expect("an X then a Y");
            let y = rest.trim_end_matches("D01*").trim_end_matches("D02*");
            corners.push([x.parse().expect("an X"), y.parse().expect("a Y")]);
        }
    }
    found
}

#[test]
fn each_teardrop_is_a_polygon_on_a_layer_and_a_net() {
    let snapshot = snapshot_of(SOURCE);
    let teardrops = snapshot["teardrops"].as_array().expect("a teardrops array");
    assert_eq!(teardrops.len(), 2, "one at each end of the track");
    for teardrop in teardrops {
        assert_eq!(teardrop["net"], "A");
        assert_eq!(teardrop["layer_mask"], 1, "the top layer");
        assert_eq!(teardrop["points"].as_array().expect("points").len(), 4);
    }
}

#[test]
fn the_corners_are_the_corners_the_gerber_region_has() {
    let snapshot = snapshot_of(SOURCE);
    let drawn: Vec<Vec<[i64; 2]>> = snapshot["teardrops"]
        .as_array()
        .expect("a teardrops array")
        .iter()
        .map(|teardrop| {
            teardrop["points"]
                .as_array()
                .expect("points")
                .iter()
                .map(|p| [p[0].as_i64().expect("x"), p[1].as_i64().expect("y")])
                .collect()
        })
        .collect();

    let mut world = cypcb_world::BoardWorld::new();
    let mut library = cypcb_world::footprint::FootprintLibrary::new();
    let parsed = cypcb_parser::parse(SOURCE);
    assert!(parsed.errors.is_empty());
    let synced = cypcb_world::sync_ast_to_world(&parsed.value, SOURCE, &mut world, &mut library);
    assert!(synced.errors.is_empty());
    let gerber = cypcb_export::gerber::copper::export_copper_layer(
        &mut world,
        &library,
        cypcb_world::Layer::TopCopper,
        &cypcb_export::coords::CoordinateFormat::FORMAT_MM_2_6,
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .expect("the top copper exports");

    assert_eq!(
        drawn,
        regions(&gerber),
        "the screen draws the polygons the fabricator is sent"
    );
}

#[test]
fn a_board_that_asks_for_none_sends_none() {
    let plain = SOURCE.replace("    teardrops\n", "");
    assert_ne!(plain, SOURCE);
    let snapshot = snapshot_of(&plain);
    assert!(
        snapshot.get("teardrops").is_none(),
        "the field is absent, so a board without fillets sends what it always sent"
    );
}
