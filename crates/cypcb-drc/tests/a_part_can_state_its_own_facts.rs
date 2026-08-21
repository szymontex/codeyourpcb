//! A part can state what its datasheet says, so an assertion can read it.
//!
//! `cargo test -p cypcb-drc --test a_part_can_state_its_own_facts`
//!
//! `assert U1.output within 3.3V +/- 0.1V` was the one assertion in
//! `examples/v2-constraints.cypcb` that stayed unanswerable after `within`
//! learned to evaluate: the checker read five things and a design can write
//! about anything. `spec { output 3.3V }` is where a part says something the
//! language has no keyword for.
//!
//! Free names inside the block, on purpose. Outside it the component block
//! stays strict, because a misspelt `at` or `value` is a mistake and a
//! misspelt datasheet fact is not something this tool can know about.

use cypcb_drc::{run_drc, DesignRules};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn load(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);
    world
}

/// A board with one part carrying `spec`, and one assertion about it.
fn assertion(spec: &str, claim: &str) -> Vec<String> {
    let spec = if spec.is_empty() {
        String::new()
    } else {
        format!("    spec {{\n        {spec}\n    }}\n")
    };
    let mut world = load(&format!(
        "version 1\n\n\
         board t {{\n    size 40mm x 20mm\n    layers 2\n}}\n\n\
         component U1 ic \"SOIC-8\" {{\n    at 10mm, 10mm\n{spec}}}\n\n\
         assert {claim}\n"
    ));
    run_drc(&mut world, &DesignRules::default())
        .violations
        .into_iter()
        .filter(|violation| violation.kind.to_string() == "assertion")
        .map(|violation| violation.message)
        .collect()
}

#[test]
fn a_stated_fact_answers_an_assertion_about_it() {
    // The example's third assertion, working.
    assert_eq!(
        assertion("output 3.3V", "U1.output within 3.3V +/- 0.1V"),
        Vec::<String>::new()
    );
}

#[test]
fn a_stated_fact_can_fail_its_assertion() {
    // The point of stating it: a part that does not do what the design assumed
    // is reported, with both numbers.
    let said = assertion("output 5V", "U1.output within 3.3V +/- 0.1V");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("assertion failed"), "{}", said[0]);
    assert!(said[0].contains("U1.output"), "{}", said[0]);
}

#[test]
fn a_comparison_reads_it_too() {
    // `within` is not the only expression. The resolver answers both.
    assert_eq!(
        assertion("quiescent 25mA", "U1.quiescent <= 50mA"),
        Vec::<String>::new()
    );
    assert_eq!(assertion("quiescent 80mA", "U1.quiescent <= 50mA").len(), 1);
}

#[test]
fn several_facts_live_side_by_side() {
    let mut world = load(
        "version 1\n\n\
         board t {\n    size 40mm x 20mm\n    layers 2\n}\n\n\
         component U1 ic \"SOIC-8\" {\n    at 10mm, 10mm\n    \
         spec {\n        output 3.3V\n        quiescent 25mA\n    }\n}\n\n\
         assert U1.output within 3.3V +/- 0.1V\n\
         assert U1.quiescent <= 50mA\n",
    );
    let said: Vec<String> = run_drc(&mut world, &DesignRules::default())
        .violations
        .into_iter()
        .filter(|violation| violation.kind.to_string() == "assertion")
        .map(|violation| violation.message)
        .collect();
    assert_eq!(said, Vec::<String>::new());
}

#[test]
fn a_fact_the_part_never_stated_is_still_refused() {
    // Stating one does not make the checker omniscient about the rest, and the
    // refusal still names what it can read.
    let said = assertion("output 3.3V", "U1.ripple <= 50mV");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("U1.ripple"), "{}", said[0]);
    assert!(
        said[0].contains("not something the checker can read"),
        "{}",
        said[0]
    );
}

#[test]
fn the_part_that_stated_it_is_the_one_that_answers() {
    // Two parts, one fact each. A lookup keyed on the name alone would answer
    // U2 with U1's number.
    let mut world = load(
        "version 1\n\n\
         board t {\n    size 40mm x 20mm\n    layers 2\n}\n\n\
         component U1 ic \"SOIC-8\" {\n    at 10mm, 10mm\n    \
         spec {\n        output 3.3V\n    }\n}\n\n\
         component U2 ic \"SOIC-8\" {\n    at 20mm, 10mm\n    \
         spec {\n        output 5V\n    }\n}\n\n\
         assert U1.output within 3.3V +/- 0.1V\n\
         assert U2.output within 5V +/- 0.1V\n",
    );
    let said: Vec<String> = run_drc(&mut world, &DesignRules::default())
        .violations
        .into_iter()
        .filter(|violation| violation.kind.to_string() == "assertion")
        .map(|violation| violation.message)
        .collect();
    assert_eq!(said, Vec::<String>::new());
}

#[test]
fn a_name_with_no_quantity_after_it_is_refused() {
    // `spec { output }` states nothing, and a fact with no number is not a
    // fact this checker can use.
    let parsed = cypcb_parser::parse(
        "version 1\n\n\
         board t {\n    size 40mm x 20mm\n    layers 2\n}\n\n\
         component U1 ic \"SOIC-8\" {\n    at 10mm, 10mm\n    spec {\n        output\n    }\n}\n",
    );
    assert!(
        !parsed.errors.is_empty(),
        "`spec {{ output }}` was accepted"
    );
}
