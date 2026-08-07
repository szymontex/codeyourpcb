//! What the viewer draws for a copper pour must be what the fabricator is sent.
//!
//! A zone as written is a rectangle. The copper made from it is that rectangle
//! minus the clearance around every other piece of copper on the layer. The
//! exporter has always sent the second one; the snapshot the viewer draws from
//! carried no zones at all, so the screen showed a bare rectangle - or nothing.
//! That hides exactly the mistakes a pour causes: a plane swallowing a pad, an
//! island cut off from the net it is supposed to be.
//!
//! These tests hold the two to the same geometry.

use cypcb_render::PcbEngine;

/// A board with a pad in the middle of a pour that is on another net.
const SOURCE: &str = r#"
board pour_test {
    size 20mm x 20mm
    layers 2
}

footprint PAD1 {
    description "one square pad"
    courtyard 2mm x 2mm

    pad 1 rect at 0mm, 0mm size 1mm x 1mm
}

component R1 resistor "PAD1" {
    value "10k"
    at 10mm, 10mm
}

net VCC {
    R1.1
}

zone gnd {
    bounds 5mm, 5mm to 15mm, 15mm
    layer top
    net GND
}
"#;

/// Load the source and hand back the pours the viewer would draw.
fn pours(source: &str) -> Vec<serde_json::Value> {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(source);
    assert!(errors.is_empty(), "source did not load: {errors}");

    let snapshot: serde_json::Value =
        serde_json::from_str(&engine.get_snapshot()).expect("snapshot is JSON");
    snapshot
        .get("pours")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default()
}

#[test]
fn the_snapshot_carries_the_pour_as_copper_not_as_its_outline() {
    let pours = pours(SOURCE);
    assert_eq!(pours.len(), 1, "one zone on one layer is one pour");

    let rects = pours[0]["rects"].as_array().expect("rectangles");
    assert!(
        rects.len() > 1,
        "a pad cut out of the middle splits the plane into several pieces, got {}",
        rects.len()
    );

    // The pad is on VCC and the pour is on GND, so no copper may cover it.
    let pad_x = 10_000_000_i64;
    let pad_y = 10_000_000_i64;
    for rect in rects {
        let r: Vec<i64> = rect
            .as_array()
            .expect("four numbers")
            .iter()
            .map(|v| v.as_i64().expect("a number"))
            .collect();
        let covers = r[0] <= pad_x && pad_x <= r[2] && r[1] <= pad_y && pad_y <= r[3];
        assert!(!covers, "the plane covers a pad on another net: {r:?}");
    }
}

#[test]
fn the_viewer_and_the_gerber_are_given_the_same_rectangles() {
    // The exporter emits one Gerber region per rectangle, so counting `G36`
    // in the top copper file counts the same geometry the snapshot carries.
    // If either side ever computes its own, this test is what notices.
    let mut engine = PcbEngine::new();
    assert!(engine.load_source(SOURCE).is_empty());

    let drawn: usize = pours(SOURCE)
        .iter()
        .map(|pour| pour["rects"].as_array().map_or(0, |r| r.len()))
        .sum();

    let mut world = cypcb_world::BoardWorld::new();
    let mut library = cypcb_world::footprint::FootprintLibrary::new();
    let parsed = cypcb_parser::parse(SOURCE);
    cypcb_world::sync_ast_to_world(&parsed.value, SOURCE, &mut world, &mut library);

    let gerber = cypcb_export::gerber::copper::export_copper_layer(
        &mut world,
        &library,
        cypcb_world::Layer::TopCopper,
        &cypcb_export::coords::CoordinateFormat::FORMAT_MM_2_6,
    )
    .expect("the top copper layer exports");

    let fabricated = gerber.matches("G36*").count();
    assert_eq!(
        drawn, fabricated,
        "the screen draws {drawn} pieces of copper and the fabricator is sent {fabricated}"
    );
}

/// The path the viewer actually takes: the host parses, the engine fills.
///
/// `WasmPcbEngineAdapter.load_source` parses in JavaScript and hands the
/// engine a snapshot, so a zone only reaches the engine if the snapshot
/// carries it. This is that round trip, in the same shape the browser sends.
#[test]
fn a_zone_sent_in_a_snapshot_comes_back_as_filled_copper() {
    let snapshot = serde_json::json!({
        "board": {
            "name": "t",
            "width_nm": 20_000_000i64,
            "height_nm": 20_000_000i64,
            "layer_count": 2
        },
        "components": [],
        "nets": [],
        "violations": [],
        "traces": [],
        "vias": [],
        "ratsnest": [],
        "zones": [{
            "name": "gnd",
            "kind": "pour",
            "layer_mask": 1,
            "net": "GND",
            "bounds": [5_000_000i64, 5_000_000i64, 15_000_000i64, 15_000_000i64]
        }]
    });

    let mut engine = PcbEngine::new();
    let errors = engine.load_snapshot_json(&snapshot.to_string());
    assert!(errors.is_empty(), "the snapshot did not load: {errors}");

    let back: serde_json::Value =
        serde_json::from_str(&engine.get_snapshot()).expect("snapshot is JSON");

    let zones = back["zones"].as_array().expect("the zones come back");
    assert_eq!(zones.len(), 1, "a zone sent in must survive the round trip");
    assert_eq!(zones[0]["net"], "GND");

    let pours = back["pours"].as_array().expect("the pours are computed");
    assert_eq!(pours.len(), 1, "one zone on one layer is one pour");
    let rects = pours[0]["rects"].as_array().expect("rectangles");
    assert_eq!(
        rects.len(),
        1,
        "an empty board leaves the pour whole, got {rects:?}"
    );
    assert_eq!(
        rects[0],
        serde_json::json!([5_000_000i64, 5_000_000i64, 15_000_000i64, 15_000_000i64]),
        "the copper is the zone itself when nothing is in its way"
    );
}

/// Two planes on different nets over the same copper have to reach the panel.
///
/// The viewer takes its violations from the engine on every `get_snapshot`, so
/// a zone rule that fires in Rust should be on screen without anything else
/// being wired. This is the check that it is - the pour is drawn now, and a
/// drawing with no warning beside it is worse than no drawing.
#[test]
fn overlapping_planes_are_reported_to_whoever_holds_the_snapshot() {
    let snapshot = serde_json::json!({
        "board": {
            "name": "t",
            "width_nm": 40_000_000i64,
            "height_nm": 40_000_000i64,
            "layer_count": 2
        },
        "components": [],
        "nets": [],
        "violations": [],
        "traces": [],
        "vias": [],
        "ratsnest": [],
        "zones": [
            {
                "name": "gnd",
                "kind": "pour",
                "layer_mask": 1,
                "net": "GND",
                "bounds": [5_000_000i64, 5_000_000i64, 20_000_000i64, 20_000_000i64]
            },
            {
                "name": "vcc",
                "kind": "pour",
                "layer_mask": 1,
                "net": "VCC",
                "bounds": [15_000_000i64, 5_000_000i64, 30_000_000i64, 20_000_000i64]
            }
        ]
    });

    let mut engine = PcbEngine::new();
    assert!(engine.load_snapshot_json(&snapshot.to_string()).is_empty());

    let back: serde_json::Value =
        serde_json::from_str(&engine.get_snapshot()).expect("snapshot is JSON");
    let violations = back["violations"].as_array().expect("violations come back");

    assert!(
        !violations.is_empty(),
        "a ground plane over a supply plane is a short and nothing said so"
    );

    // And the control, so this is not a test that passes on any board: the
    // same two planes moved apart say nothing.
    let apart = snapshot.to_string().replace(
        "[15000000,5000000,30000000,20000000]",
        "[25000000,5000000,30000000,20000000]",
    );
    let mut engine = PcbEngine::new();
    assert!(engine.load_snapshot_json(&apart).is_empty());
    let back: serde_json::Value =
        serde_json::from_str(&engine.get_snapshot()).expect("snapshot is JSON");
    assert!(
        back["violations"]
            .as_array()
            .expect("violations come back")
            .is_empty(),
        "two planes that do not touch are not a fault"
    );
}
