//! A typo in a pin number used to cost a connection and say nothing.
//!
//! `cargo test -p cypcb-world --test a_net_cannot_name_a_pin_that_does_not_exist`
//!
//! `net SIG { R1.3 }` on a two-pad part stored a connection to pin 3. The
//! ratsnest then had one end and nothing to route, and the only thing `cypcb
//! check` reported was that R1.1 and R1.2 were unconnected - which reads as the
//! design's own fault rather than as a typo three characters long. The
//! connection the user asked for simply did not exist, and nothing said so.
//!
//! The check is deliberately narrow: it fires only when the footprint is known.
//! A part fetched from a supplier has no pads until its fetch lands, and
//! erroring on those would refuse every board that uses one.

use cypcb_parser::parse;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn sync(source: &str) -> Vec<String> {
    let parsed = parse(source);
    assert!(
        parsed.errors.is_empty(),
        "the board must parse: {:?}",
        parsed.errors
    );

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    sync_ast_to_world(&parsed.value, source, &mut world, &mut library)
        .errors
        .iter()
        .map(|e| e.to_string())
        .collect()
}

const TWO_PAD_PART: &str = r#"
board demo {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 5mm, 10mm
}
"#;

#[test]
fn a_pin_the_footprint_does_not_have_is_an_error() {
    let errors = sync(&format!("{TWO_PAD_PART}\nnet SIG {{\n    R1.3\n}}\n"));

    let named = errors
        .iter()
        .find(|e| e.contains("has no pin"))
        .unwrap_or_else(|| panic!("a pin that does not exist has to be reported: {errors:?}"));

    // The message has to say which part, which pin, and what it could have
    // meant - a bare "invalid pin" sends the reader back to the datasheet.
    assert!(named.contains("'R1'"), "names the part: {named}");
    assert!(named.contains("'3'"), "names the pin asked for: {named}");
    assert!(named.contains("1, 2"), "names the pins it has: {named}");
}

#[test]
fn the_pins_the_part_does_have_are_not_an_error() {
    let errors = sync(&format!(
        "{TWO_PAD_PART}\nnet SIG {{\n    R1.1\n    R1.2\n}}\n"
    ));
    assert!(
        errors.is_empty(),
        "a net wiring real pins has nothing wrong with it: {errors:?}"
    );
}

#[test]
fn a_part_whose_footprint_is_not_in_the_library_is_left_alone() {
    // The narrowness that makes this safe. An unknown footprint is already
    // reported once, by `UnknownFootprint`; reporting every pin of it as
    // missing would bury that under noise, and a supplier's part legitimately
    // has no pads until its fetch lands.
    let source = r#"
board demo {
    size 20mm x 20mm
    layers 2
}

component U1 ic "NOSUCH-99" {
    value "x"
    at 5mm, 5mm
}

net SIG {
    U1.7
}
"#;
    let errors = sync(source);

    assert!(
        errors.iter().any(|e| e.contains("unknown footprint")),
        "the missing footprint is still reported: {errors:?}"
    );
    assert!(
        !errors.iter().any(|e| e.contains("has no pin")),
        "a footprint nobody has cannot say which pins are wrong: {errors:?}"
    );
}
