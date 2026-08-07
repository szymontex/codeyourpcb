//! A DRC violation has to know which definition it is about.
//!
//! `cargo test -p cypcb-render --features native --test a_violation_knows_its_line`
//!
//! Violations are discovered in board coordinates - two pieces of copper too
//! close - and the editor needs a line. The rules never fill
//! `DrcViolation::source_span`; all seventeen construction sites pass `None`.
//! The route that does exist is the entity the violation names and the span
//! sync attached to it, and nothing was walking it: every DRC marker in the
//! browser sat on line 1, which put the whole report on the `board` keyword.

use cypcb_render::PcbEngine;

/// Two 0805 parts overlapping. R2's definition starts on line 11.
const OVERLAPPING: &str = r#"board demo {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 10mm, 10mm
}

component R2 resistor "0805" {
    value "10k"
    at 10.2mm, 10mm
}
"#;

#[test]
fn a_violation_points_at_the_part_it_is_about() {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(OVERLAPPING);
    assert!(errors.is_empty(), "the board must load: {errors:?}");

    let violations: Vec<serde_json::Value> =
        serde_json::from_str(&engine.get_violations_json()).expect("violations are JSON");
    assert!(
        !violations.is_empty(),
        "overlapping parts have to violate something"
    );

    let located: Vec<&serde_json::Value> = violations
        .iter()
        .filter(|v| v.get("line").is_some())
        .collect();
    assert!(
        !located.is_empty(),
        "at least one violation has to name the line of the part it is about; got {violations:#?}"
    );

    // R1 is defined on line 6 and R2 on line 11. Every located violation has
    // to point at one of them, not at line 1.
    for violation in &located {
        let line = violation["line"].as_u64().expect("a number");
        assert!(
            line == 6 || line == 11,
            "a violation about R1 or R2 should point at line 6 or 11, got {line} for {violation}"
        );
    }
}

#[test]
fn a_board_with_nothing_wrong_locates_nothing() {
    let mut engine = PcbEngine::new();
    engine.load_source("board demo {\n    size 20mm x 20mm\n    layers 2\n}\n");
    let violations: Vec<serde_json::Value> =
        serde_json::from_str(&engine.get_violations_json()).expect("violations are JSON");
    assert!(
        violations.is_empty(),
        "an empty board violates nothing: {violations:#?}"
    );
}
