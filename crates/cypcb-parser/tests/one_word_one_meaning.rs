//! The same property name has to mean the same thing wherever it is written.
//!
//! A zone took `layer top` and a trace took `layer Top`, so one capital letter
//! decided whether a line was correct or a syntax error, depending on which
//! block it sat in. Found while wiring zones into the viewer, where the
//! parser's `net GND` inside a zone was read as the start of a net block: the
//! same three words meaning two things is a trap the guide can document but
//! not fix.

use cypcb_parser::parse;

fn parses(source: &str) -> bool {
    parse(source).errors.is_empty()
}

const BOARD: &str = "board t {\n    size 40mm x 40mm\n    layers 2\n}\n";

#[test]
fn a_zone_takes_either_spelling_of_its_layer() {
    for spelling in ["top", "Top"] {
        let source = format!(
            "{BOARD}\nzone gnd {{\n    bounds 5mm, 5mm to 35mm, 35mm\n    layer {spelling}\n}}\n"
        );
        assert!(parses(&source), "a zone rejected `layer {spelling}`");
    }
}

#[test]
fn a_trace_takes_either_spelling_of_its_layer() {
    for spelling in ["Top", "top"] {
        let source = format!(
            "{BOARD}\ntrace SIG {{\n    path 2mm, 2mm -> 8mm, 8mm\n    layer {spelling}\n    width 0.2mm\n}}\n"
        );
        assert!(parses(&source), "a trace rejected `layer {spelling}`");
    }
}

#[test]
fn a_net_inside_a_zone_is_the_pour_net_and_not_a_new_net_block() {
    // `net GND` opens a net block at the top level and names the pour's net
    // inside a zone. Both readings parse; only one is right, and the viewer's
    // own parser had the wrong one.
    let source = format!(
        "{BOARD}\nzone gnd {{\n    bounds 5mm, 5mm to 35mm, 35mm\n    layer top\n    net GND\n}}\n"
    );
    let result = parse(&source);
    assert!(result.errors.is_empty(), "the zone should parse");

    let zones = result
        .value
        .definitions
        .iter()
        .filter(|definition| matches!(definition, cypcb_parser::ast::Definition::Zone(_)))
        .count();
    let nets = result
        .value
        .definitions
        .iter()
        .filter(|definition| matches!(definition, cypcb_parser::ast::Definition::Net(_)))
        .count();

    assert_eq!(zones, 1, "one zone");
    assert_eq!(nets, 0, "and no net definition - that `net` belongs to the zone");
}
