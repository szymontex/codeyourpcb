//! An assertion with a band is answered, not deferred.
//!
//! `cargo test -p cypcb-drc --test an_assertion_with_a_band_is_answered`
//!
//! `within` used to reply "not checked: the board model does not carry
//! tolerances yet" to every question. That made `examples/v2-constraints.cypcb`,
//! the example whose whole subject is assertions, report three violations for
//! the three assertions it exists to demonstrate.
//!
//! That answer was reading a harder question than the one asked. A part's own
//! manufacturing tolerance really is not in the model. `R1.value within 10kohm
//! +/- 5%` does not need it: it asks whether the value the design **states**
//! falls in a band, and the value and the band are both in the file.
//!
//! What is still refused is refused for a reason that names itself.

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

/// A board with one resistor of `value`, and one assertion about it.
fn assertion(value: &str, claim: &str) -> Vec<String> {
    let mut world = load(&format!(
        "version 1\n\n\
         board t {{\n    size 40mm x 20mm\n    layers 2\n}}\n\n\
         component R1 resistor \"0402\" {{\n    value {value}\n    at 10mm, 10mm\n}}\n\n\
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
fn a_value_inside_a_percentage_band_holds() {
    assert_eq!(
        assertion("10kohm", "R1.value within 10kohm +/- 5%"),
        Vec::<String>::new()
    );
    // And the edges of the band are inside it: 5% of 10k is 500 ohm.
    assert_eq!(
        assertion("10.5kohm", "R1.value within 10kohm +/- 5%"),
        Vec::<String>::new()
    );
    assert_eq!(
        assertion("9.5kohm", "R1.value within 10kohm +/- 5%"),
        Vec::<String>::new()
    );
}

#[test]
fn a_value_outside_the_band_fails_and_names_both_ends() {
    let said = assertion("12kohm", "R1.value within 10kohm +/- 5%");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("assertion failed"), "{}", said[0]);
    // A reader deciding what to change needs the band, not just a verdict.
    assert!(said[0].contains("outside"), "{}", said[0]);
    // Printed in base units, which is what the rest of this checker does.
    assert!(said[0].contains("9500ohm"), "the low end: {}", said[0]);
    assert!(said[0].contains("10500ohm"), "the high end: {}", said[0]);
}

#[test]
fn a_range_band_runs_from_the_nominal_to_the_stated_end() {
    // `within 100nF to 220nF` is not a spread either side of 100nF. The
    // nominal is the low end.
    assert_eq!(
        assertion("100nF", "R1.value within 100nF to 220nF"),
        Vec::<String>::new()
    );
    assert_eq!(
        assertion("220nF", "R1.value within 100nF to 220nF"),
        Vec::<String>::new()
    );
    assert_eq!(
        assertion("150nF", "R1.value within 100nF to 220nF"),
        Vec::<String>::new()
    );
    assert_eq!(
        assertion("90nF", "R1.value within 100nF to 220nF").len(),
        1,
        "below the nominal is outside the range"
    );
}

#[test]
fn an_absolute_band_is_a_spread_either_side() {
    assert_eq!(
        assertion("10.05kohm", "R1.value within 10kohm +/- 0.1kohm"),
        Vec::<String>::new()
    );
    assert_eq!(
        assertion("10.2kohm", "R1.value within 10kohm +/- 0.1kohm").len(),
        1
    );
}

#[test]
fn a_within_with_no_band_says_what_a_band_looks_like() {
    let said = assertion("10kohm", "R1.value within 10kohm");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("not checked"), "{}", said[0]);
    assert!(said[0].contains("+/- 5%"), "{}", said[0]);
    assert!(said[0].contains("to 220nF"), "{}", said[0]);
}

#[test]
fn two_different_quantities_are_refused_rather_than_compared() {
    // A resistance is not a capacitance whatever the arithmetic says.
    let said = assertion("10kohm", "R1.value within 100nF +/- 5%");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("not checked"), "{}", said[0]);
    assert!(said[0].contains("cannot be compared"), "{}", said[0]);
}

#[test]
fn what_the_checker_cannot_read_is_named_rather_than_blamed_on_within() {
    // `U1.output` is the third assertion in the example, and it is still
    // refused - but for what it is, with the list of what the checker does
    // know beside it.
    let mut world = load(
        "version 1\n\n\
         board t {\n    size 40mm x 20mm\n    layers 2\n}\n\n\
         component U1 ic \"SOIC-8\" {\n    at 10mm, 10mm\n}\n\n\
         assert U1.output within 3.3V +/- 0.1V\n",
    );
    let said: Vec<String> = run_drc(&mut world, &DesignRules::default())
        .violations
        .into_iter()
        .filter(|violation| violation.kind.to_string() == "assertion")
        .map(|violation| violation.message)
        .collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("U1.output"), "{}", said[0]);
    assert!(
        !said[0].contains("does not carry"),
        "the old blanket answer is gone: {}",
        said[0]
    );
}
