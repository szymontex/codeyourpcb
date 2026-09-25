//! The editor's router routes the parts drawn with a footprint the design
//! defines, and its count of what it left agrees with its checker.
//!
//! `cargo test -p cypcb-render --test the_editor_routes_the_footprints_the_design_defines`
//!
//! The engine keeps the library it synchronised with and routes with that
//! one. Two routing tests once did not, and the router said Complete over the
//! LED of `examples/blink.cypcb`, whose footprint `LED_0805` the design
//! defines. This holds every routing button the viewer has to the same board.

use cypcb_render::PcbEngine;

fn blink() -> PcbEngine {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/blink.cypcb");
    let source = std::fs::read_to_string(path).expect("the example is on disk");
    assert!(source.contains("footprint LED_0805 {"), "the premise");
    let mut engine = PcbEngine::new();
    let error = engine.load_source(&source);
    assert!(error.is_empty(), "the example loads: {error}");
    engine
}

/// The messages of the pins the checker finds no copper on.
fn unrouted_pins(engine: &PcbEngine) -> Vec<String> {
    let violations: Vec<serde_json::Value> =
        serde_json::from_str(&engine.get_violations_json()).expect("the engine writes JSON");
    violations
        .iter()
        .filter_map(|v| v["message"].as_str())
        .filter(|message| message.contains("no copper reaches"))
        .map(str::to_string)
        .collect()
}

fn status(json: &str) -> serde_json::Value {
    serde_json::from_str(json).expect("the router answers in JSON")
}

#[test]
fn before_routing_the_checker_sees_the_leds_pins_bare() {
    // The control: the checker reads the LED's pads from the same library, so
    // a bare LED is visible to it. Without this an empty list below would
    // prove nothing.
    let mut engine = blink();
    engine.run_drc_incremental();
    let left = unrouted_pins(&engine);
    assert!(left.iter().any(|m| m.contains("LED1.A")), "{left:#?}");
    assert!(left.iter().any(|m| m.contains("LED1.K")), "{left:#?}");
}

#[test]
fn auto_route_routes_the_led_and_counts_what_it_left() {
    fn plain(engine: &mut PcbEngine) -> String {
        engine.auto_route()
    }
    fn with_params(engine: &mut PcbEngine) -> String {
        engine.auto_route_with_params("{}".to_string())
    }
    type Road = fn(&mut PcbEngine) -> String;
    let roads: [(&str, Road); 2] = [
        ("auto_route", plain),
        ("auto_route_with_params", with_params),
    ];
    for (road, json) in roads {
        let mut engine = blink();
        let said = status(&json(&mut engine));
        assert_eq!(said["ok"], true, "{road}: {said}");
        let left = unrouted_pins(&engine);
        assert!(
            left.iter().all(|m| !m.contains("LED1.")),
            "{road} left the LED's pins: {left:#?}"
        );
        assert_eq!(
            said["unrouted"].as_u64() == Some(0),
            left.is_empty(),
            "{road}: the router said {said} and the checker found {left:#?}"
        );
    }
}

#[test]
fn auto_route_variants_routes_the_led() {
    let mut engine = blink();
    let results = status(&engine.auto_route_variants());
    let best = &results[0];
    let left = unrouted_pins(&engine);
    assert!(
        left.iter().all(|m| !m.contains("LED1.")),
        "the best variant left the LED's pins: {left:#?}"
    );
    assert_eq!(
        best["unrouted"].as_u64() == Some(0),
        left.is_empty(),
        "the best variant said {} and the checker found {left:#?}",
        best["unrouted"]
    );
}
