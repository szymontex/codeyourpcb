//! A claim a design makes about itself has to survive being saved.
//!
//! `cargo test -p cypcb-world --test an_assertion_survives_being_written_down`
//!
//! `assert R1.value >= 10kohm` is a rule: the checker reports a failed one as
//! an `assertion` violation, and `examples/v2-constraints.cypcb` exists to
//! demonstrate it. `board_as_dsl` wrote the parts and dropped every claim made
//! about them, so a board saved through this writer came back unable to fail
//! the way its author intended - a rule lost, like the differential pair
//! before it, rather than the brevity a `netclass` costs.

use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

const CLAIMED: &str = r#"version 1

board claimed {
    size 40mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value 10kohm
    at 10mm, 10mm
}

component R2 resistor "0402" {
    value 4.7kohm
    at 30mm, 10mm
}

assert R1.value >= 10kohm

assert R2.value within 5kohm
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

#[test]
fn both_kinds_of_claim_come_back() {
    let mut before = load(CLAIMED);
    assert_eq!(
        before.assertions().len(),
        2,
        "a comparison and a within, as the grammar has them"
    );

    let written = board_as_dsl(&mut before);
    assert!(
        written.contains("assert R1.value >= 10kohm"),
        "the comparison is written with its operator and its unit:\n{written}"
    );
    assert!(
        written.contains("assert R2.value within 5kohm"),
        "and the within form with its target:\n{written}"
    );

    let after = load(&written);
    assert_eq!(
        after.assertions().len(),
        2,
        "the file this project wrote is a file it reads:\n{written}"
    );
}
