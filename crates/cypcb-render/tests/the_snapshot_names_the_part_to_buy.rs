//! The browser reads the part number off the model, not out of the text.
//!
//! `cargo test -p cypcb-render --features native --test the_snapshot_names_the_part_to_buy`
//!
//! `lcsc "C7593"` used to reach the viewer through a regular expression over
//! the raw source, because the language did not have the property and the
//! model never saw it. That is a second reader of the language - the thing
//! `docs/one-parser.md` exists to prevent - and it could not see a part inside
//! a module, missed any part number that was not `C` followed by digits, and
//! gave up on a component block containing a nested brace.

use cypcb_render::PcbEngine;

const BOARD: &str = r#"version 1

board parts {
    size 30mm x 20mm
    layers 2
}

component U1 ic "SOIC-8" {
    value "NE555"
    lcsc "C7593"
    at 10mm, 10mm
}

component R1 resistor "0402" {
    value "10k"
    at 20mm, 10mm
}

module Sensor {
    pin OUT

    component U2 ic "SOIC-8" {
        value "TMP102"
        lcsc "C84291"
        at 0mm, 0mm
    }

    net OUT {
        U2.1
    }
}

use Sensor as S1 at 25mm, 15mm {
    OUT = SIG
}
"#;

fn snapshot(source: &str) -> serde_json::Value {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(source);
    assert_eq!(errors, "", "the fixture loads");
    serde_json::from_str(&engine.get_snapshot()).expect("the snapshot is JSON")
}

#[test]
fn a_part_the_design_names_reaches_the_snapshot() {
    let snapshot = snapshot(BOARD);
    let components = snapshot["components"].as_array().expect("components");

    let u1 = components
        .iter()
        .find(|c| c["refdes"] == "U1")
        .expect("U1 is on the board");
    assert_eq!(u1["lcsc"], "C7593");
}

#[test]
fn a_component_that_names_no_part_says_nothing() {
    // Absent rather than an empty string: the host asks "did the design name a
    // part", and "" is not an answer to that.
    let snapshot = snapshot(BOARD);
    let components = snapshot["components"].as_array().expect("components");

    let r1 = components
        .iter()
        .find(|c| c["refdes"] == "R1")
        .expect("R1 is on the board");
    assert!(r1.get("lcsc").is_none(), "R1 names no part: {r1}");
}

#[test]
fn a_part_inside_a_module_reaches_the_snapshot_too() {
    // The case the regular expression could not see at all: a module's
    // components arrive under the instance name, and their part numbers with
    // them.
    let snapshot = snapshot(BOARD);
    let components = snapshot["components"].as_array().expect("components");

    let inside = components
        .iter()
        .find(|c| c["refdes"] == "S1_U2")
        .expect("the module's part is placed as S1_U2");
    assert_eq!(inside["lcsc"], "C84291");
}
