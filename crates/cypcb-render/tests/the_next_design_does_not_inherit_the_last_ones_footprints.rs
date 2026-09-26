//! Every load builds the engine's footprint library from scratch - the
//! built-ins and what the host fetched - and the checker and the router both
//! read that one library.
//!
//! `cargo test -p cypcb-render --test the_next_design_does_not_inherit_the_last_ones_footprints`
//!
//! Until 2026-09-26 the engine kept one library for its whole life. A
//! snapshot's footprints and a KiCad file's stayed in it, so the next `.cypcb`
//! could name a footprint it never defined and load without a word, routed
//! over pads from another board. A snapshot also gave its footprints to the
//! router and not to the checker, and the debug router left out the pins the
//! library had no pad for, which `auto_route` counts.

use cypcb_render::PcbEngine;

fn blink() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/blink.cypcb");
    let source = std::fs::read_to_string(path).expect("the example is on disk");
    assert!(source.contains("footprint LED_0805 {"), "the premise");
    source
}

/// Blink with its `LED_0805` block taken out: LED1 still names it.
fn blink_naming_a_footprint_it_does_not_define() -> String {
    let source = blink();
    let start = source.find("footprint LED_0805 {").unwrap();
    let end = start + source[start..].find("\n}\n").unwrap() + 3;
    format!("{}{}", &source[..start], &source[end..])
}

/// A KiCad board whose one part has a footprint no built-in is called.
const KICAD: &str = r#"(kicad_pcb (version 20240108) (generator "hand-written-test")
  (general (thickness 1.6))
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (net 1 "SIG")
  (gr_rect (start 0 0) (end 30 20) (layer "Edge.Cuts") (width 0.05))
  (footprint "Connector:ODD_PART"
    (at 10 10)
    (property "Reference" "J1")
    (pad "1" smd rect (at -1 0) (size 1 1) (layers "F.Cu") (net 1 "SIG"))
  )
  (footprint "Resistor_SMD:R_0402"
    (at 20 10)
    (property "Reference" "R1")
    (pad "1" smd rect (at -0.5 0) (size 0.5 0.5) (layers "F.Cu") (net 1 "SIG"))
  )
)
"#;

/// A `.cypcb` naming the KiCad board's footprint without defining it.
fn source_naming(footprint: &str) -> String {
    format!(
        "version 1\n\nboard b {{\n    size 30mm x 20mm\n    layers 2\n}}\n\n\
         component J1 connector \"{footprint}\" {{\n    at 10mm, 10mm\n}}\n\n\
         component R1 resistor \"0402\" {{\n    at 20mm, 10mm\n}}\n\n\
         net SIG {{\n    J1.1\n    R1.1\n}}\n"
    )
}

/// The pins of `part` the checker finds bare. The checker reads the world's
/// library: a pin whose pad it does not know it cannot call bare.
fn bare_pins_the_checker_sees(engine: &mut PcbEngine, part: &str) -> Vec<String> {
    engine.run_drc_incremental();
    let violations: Vec<serde_json::Value> =
        serde_json::from_str(&engine.get_violations_json()).expect("the engine writes JSON");
    violations
        .iter()
        .filter_map(|v| v["message"].as_str())
        .filter(|m| m.starts_with(&format!("{part}.")) && m.contains("no copper reaches"))
        .map(str::to_string)
        .collect()
}

/// The pads of `part` in the engine's own library - the one the router gets.
fn pads_the_router_sees(engine: &mut PcbEngine, part: &str) -> usize {
    let snapshot: serde_json::Value =
        serde_json::from_str(&engine.get_snapshot()).expect("the engine writes JSON");
    snapshot["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["refdes"] == part)
        .and_then(|c| c["pads"].as_array())
        .map_or(0, Vec::len)
}

fn footprint_of(engine: &mut PcbEngine, part: &str) -> String {
    let snapshot: serde_json::Value = serde_json::from_str(&engine.get_snapshot()).unwrap();
    snapshot["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["refdes"] == part)
        .and_then(|c| c["footprint"].as_str())
        .unwrap()
        .to_string()
}

/// After `first`, loading a source that names `footprint` for `part` without
/// defining it: nothing of the first board may answer for it.
fn the_second_load_does_not_see(
    first: &str,
    mut engine: PcbEngine,
    source: &str,
    footprint: &str,
    part: &str,
) {
    let answer = engine.load_source(source);
    assert!(
        answer.contains(&format!("unknown footprint: '{footprint}'")),
        "after {first} the next design resolved {footprint} it never defined: {answer:?}"
    );
    assert_eq!(
        pads_the_router_sees(&mut engine, part),
        0,
        "after {first} the router has {part}'s pads"
    );
    let bare = bare_pins_the_checker_sees(&mut engine, part);
    assert!(
        bare.is_empty(),
        "after {first} the checker has {part}'s pads: {bare:#?}"
    );
}

#[test]
fn a_design_does_not_inherit_the_footprints_of_the_load_before_it() {
    let second = blink_naming_a_footprint_it_does_not_define();

    // From a source that defined it.
    let mut engine = PcbEngine::new();
    assert!(engine.load_source(&blink()).is_empty());
    assert_eq!(pads_the_router_sees(&mut engine, "LED1"), 2, "the control");
    assert_eq!(
        bare_pins_the_checker_sees(&mut engine, "LED1").len(),
        2,
        "the control"
    );
    the_second_load_does_not_see("a source", engine, &second, "LED_0805", "LED1");

    // From a snapshot that carried its pads.
    let snapshot = {
        let mut drawn = PcbEngine::new();
        assert!(drawn.load_source(&blink()).is_empty());
        drawn.get_snapshot()
    };
    let mut engine = PcbEngine::new();
    assert!(engine.load_snapshot_json(&snapshot).is_empty());
    assert_eq!(pads_the_router_sees(&mut engine, "LED1"), 2, "the control");
    the_second_load_does_not_see("a snapshot", engine, &second, "LED_0805", "LED1");

    // From a KiCad board, whose footprints the file itself carries.
    let mut engine = PcbEngine::new();
    assert!(engine.load_kicad(KICAD).is_empty());
    let footprint = footprint_of(&mut engine, "J1");
    assert_eq!(pads_the_router_sees(&mut engine, "J1"), 1, "the control");
    assert_eq!(
        bare_pins_the_checker_sees(&mut engine, "J1").len(),
        1,
        "the control"
    );
    the_second_load_does_not_see(
        "a KiCad board",
        engine,
        &source_naming(&footprint),
        &footprint,
        "J1",
    );
}

#[test]
fn a_snapshot_does_not_inherit_the_footprints_of_the_load_before_it() {
    // A snapshot names a footprint and carries no pads for it, as one from a
    // host that does not know the part does.
    let snapshot = {
        let mut drawn = PcbEngine::new();
        assert!(!drawn
            .load_source(&blink_naming_a_footprint_it_does_not_define())
            .is_empty());
        assert_eq!(footprint_of(&mut drawn, "LED1"), "LED_0805", "the premise");
        assert_eq!(pads_the_router_sees(&mut drawn, "LED1"), 0, "the premise");
        drawn.get_snapshot()
    };
    let mut engine = PcbEngine::new();
    assert!(engine.load_source(&blink()).is_empty());
    assert_eq!(pads_the_router_sees(&mut engine, "LED1"), 2, "the control");
    assert!(engine.load_snapshot_json(&snapshot).is_empty());
    assert_eq!(
        pads_the_router_sees(&mut engine, "LED1"),
        0,
        "the router has LED1's pads"
    );
    let bare = bare_pins_the_checker_sees(&mut engine, "LED1");
    assert!(bare.is_empty(), "the checker has LED1's pads: {bare:#?}");
}

#[test]
fn a_snapshot_gives_the_checker_the_footprints_it_gives_the_router() {
    let snapshot = {
        let mut drawn = PcbEngine::new();
        assert!(drawn.load_source(&blink()).is_empty());
        drawn.get_snapshot()
    };
    let mut engine = PcbEngine::new();
    assert!(engine.load_snapshot_json(&snapshot).is_empty());
    assert_eq!(pads_the_router_sees(&mut engine, "LED1"), 2);
    let bare = bare_pins_the_checker_sees(&mut engine, "LED1");
    assert_eq!(
        bare.len(),
        2,
        "the checker does not know LED1's pads: {bare:#?}"
    );
}

#[test]
fn a_fetched_footprint_outlives_every_kind_of_load() {
    // What the host fetched is not the last design's: the host registers once
    // and re-parses many times.
    const PADS: &str = r#"[
        {"number":"1","shape":"rect","x_nm":-500000,"y_nm":0,"width_nm":600000,"height_nm":500000,"layer_mask":1},
        {"number":"2","shape":"rect","x_nm":500000,"y_nm":0,"width_nm":600000,"height_nm":500000,"layer_mask":1}
    ]"#;
    let mut engine = PcbEngine::new();
    assert!(engine
        .register_footprint("FETCHED_PART", PADS, "")
        .is_empty());
    let snapshot = {
        let mut drawn = PcbEngine::new();
        assert!(drawn.load_source(&blink()).is_empty());
        drawn.get_snapshot()
    };
    assert!(engine.load_snapshot_json(&snapshot).is_empty());
    assert!(engine.load_kicad(KICAD).is_empty());
    let answer = engine.load_source(&source_naming("FETCHED_PART"));
    assert!(
        answer.is_empty(),
        "the fetched footprint was lost: {answer}"
    );
    assert_eq!(pads_the_router_sees(&mut engine, "J1"), 2);
    assert_eq!(bare_pins_the_checker_sees(&mut engine, "J1").len(), 1);
}

#[test]
fn the_debug_router_counts_the_pins_it_could_not_see() {
    let status = |json: String| -> serde_json::Value { serde_json::from_str(&json).unwrap() };

    // The control: every pad known, the debug router leaves nothing.
    let mut engine = PcbEngine::new();
    assert!(engine.load_source(&blink()).is_empty());
    assert_eq!(
        status(engine.auto_route_debug("{}".into()))["unrouted_count"],
        0
    );

    // LED1's two pins have no pad: the loop never saw them, and it may not
    // say it left nothing.
    let mut engine = PcbEngine::new();
    assert!(!engine
        .load_source(&blink_naming_a_footprint_it_does_not_define())
        .is_empty());
    let debug = status(engine.auto_route_debug("{}".into()));
    let unrouted = debug["unrouted_count"].as_u64().unwrap();
    assert!(
        unrouted >= 2,
        "the debug router left LED1 out of its count: {unrouted}"
    );
}
