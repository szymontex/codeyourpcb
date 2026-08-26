//! A net written down and read back has to ask for the same things.
//!
//! `cargo test -p cypcb-world --test what_a_net_asks_for_survives_being_written_down`
//!
//! `board_as_dsl` learned to write a net's constraint block on 2026-08-26, and
//! the case that proved it went through KiCad and covered three of the five
//! figures. The other two are written by the same function and nothing read
//! them back: `impedance` is kept in hundredths of an ohm and printed in ohms,
//! which is a conversion, and `neck` is two dimensions the grammar makes
//! compulsory together - a writer that emits one of them produces a file this
//! project's own parser rejects.
//!
//! This is the loop with no KiCad in it: source, model, source, model.

use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::registry::NetConstraints;
use cypcb_world::{sync_ast_to_world, BoardWorld};

const ASKING: &str = r#"version 1

board asks {
    size 40mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 30mm, 10mm
}

net SIG [width 0.5mm clearance 0.3mm current 500mA impedance 50ohm neck 0.15mm for 1mm] {
    R1.1
    R2.1
}
"#;

fn load(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);
    world
}

/// What the only net of the board asks for.
fn asks(world: &mut BoardWorld) -> NetConstraints {
    let id = world
        .nets()
        .find(|(_, name)| *name == "SIG")
        .map(|(id, _)| id)
        .expect("the board has a net called SIG");
    world
        .net_constraints(id)
        .expect("a net that asks for something has constraints")
}

#[test]
fn every_figure_comes_back() {
    let mut before = load(ASKING);
    let stated = asks(&mut before);

    // Each figure, so a failure says which one was lost rather than that the
    // struct differs.
    assert_eq!(stated.width.map(|w| w.to_mm()), Some(0.5));
    assert_eq!(stated.clearance.map(|c| c.to_mm()), Some(0.3));
    assert_eq!(stated.current_ma, Some(500.0));
    assert_eq!(stated.impedance_ohms_x100, Some(5000));
    assert_eq!(
        stated
            .neck
            .map(|neck| (neck.width.to_mm(), neck.length.to_mm())),
        Some((0.15, 1.0))
    );

    let written = board_as_dsl(&mut before);
    assert!(
        written.contains("impedance 50ohm"),
        "the model keeps hundredths and the language reads ohms:\n{written}"
    );
    assert!(
        written.contains("for "),
        "both halves of a neck or neither - one alone is a file the reader refuses:\n{written}"
    );

    // The half that matters: the file this project wrote is a file it reads.
    let mut after = load(&written);
    assert_eq!(asks(&mut after), stated, "written:\n{written}");
}
