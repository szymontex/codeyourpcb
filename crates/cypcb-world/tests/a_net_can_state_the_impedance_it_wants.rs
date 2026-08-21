//! A net can state the impedance it wants.
//!
//! `cargo test -p cypcb-world --test a_net_can_state_the_impedance_it_wants`
//!
//! `require_impedance_control` has been a flag in `cypcb-rules` with no code
//! behind it since it was written, and the reason is upstream of the checker:
//! nothing in the language could say what a net is meant to present. `width`,
//! `clearance` and `current` were the whole of a constraint block.
//!
//! The unit is compulsory. `impedance 90` would read like a width to anyone
//! scanning the line, and every other constraint in that block carries one.

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

const BOARD: &str = r#"version 1

board t {
    size 40mm x 20mm
    layers 4
}

component R1 resistor "0402" {
    value 10kohm
    at 10mm, 10mm
}

component R2 resistor "0402" {
    value 10kohm
    at 30mm, 10mm
}
"#;

fn design(extra: &str) -> String {
    format!("{BOARD}\n{extra}")
}

#[test]
fn a_netclass_carries_a_target_to_every_net_in_it() {
    let mut world = load(&design(
        "net USB_DP {\n    R1.1\n    R2.1\n}\n\n\
         net USB_DM {\n    R1.2\n    R2.2\n}\n\n\
         netclass USB [width 0.2mm impedance 90ohm] {\n    USB_DP\n    USB_DM\n}\n",
    ));
    for name in ["USB_DP", "USB_DM"] {
        let net = world.intern_net(name);
        let constraints = world
            .net_constraints(net)
            .unwrap_or_else(|| panic!("{name} carries the class"));
        assert_eq!(
            constraints.impedance_ohms_x100,
            Some(9_000),
            "{name} wants 90 ohm"
        );
    }
}

#[test]
fn a_net_can_state_one_on_its_own() {
    let mut world = load(&design(
        "net CLK [impedance 50ohm] {\n    R1.1\n    R2.1\n}\n",
    ));
    let net = world.intern_net("CLK");
    assert_eq!(
        world
            .net_constraints(net)
            .and_then(|c| c.impedance_ohms_x100),
        Some(5_000)
    );
}

#[test]
fn a_fractional_target_keeps_its_hundredths() {
    // Hundredths is the resolution both sides of the eventual comparison use,
    // and a differential pair is often specified at 90 or 100 with a half in
    // between on the single-ended side.
    let mut world = load(&design(
        "net CLK [impedance 45.5ohm] {\n    R1.1\n    R2.1\n}\n",
    ));
    let net = world.intern_net("CLK");
    assert_eq!(
        world
            .net_constraints(net)
            .and_then(|c| c.impedance_ohms_x100),
        Some(4_550)
    );
}

#[test]
fn the_unit_is_compulsory() {
    let parsed = cypcb_parser::parse(&design("net CLK [impedance 50] {\n    R1.1\n    R2.1\n}\n"));
    assert!(
        !parsed.errors.is_empty(),
        "`impedance 50` was accepted with no unit"
    );
}

#[test]
fn zero_and_below_are_refused_rather_than_stored() {
    // A net with no impedance is not a net, and a nonsense figure kept in the
    // model reads later as a target somebody chose.
    for bad in ["0ohm", "-5ohm"] {
        let source = design(&format!("net CLK [impedance {bad}] {{\n    R1.1\n}}\n"));
        let parsed = cypcb_parser::parse(&source);
        assert!(!parsed.errors.is_empty(), "`impedance {bad}` was accepted");
    }
}

#[test]
fn a_net_that_states_nothing_carries_no_target() {
    let mut world = load(&design("net CLK {\n    R1.1\n    R2.1\n}\n"));
    let net = world.intern_net("CLK");
    assert_eq!(
        world
            .net_constraints(net)
            .and_then(|c| c.impedance_ohms_x100),
        None,
        "silence is not fifty ohms"
    );
}

#[test]
fn the_refusal_names_impedance_among_what_a_block_takes() {
    // A reader who writes `impedence` should be told the word, not left to
    // guess which three the block knew about before this.
    let parsed = cypcb_parser::parse(&design("net CLK [impedence 50ohm] {\n    R1.1\n}\n"));
    let complaint = format!("{:?}", parsed.errors);
    assert!(
        complaint.contains("impedance"),
        "the refusal has to list it: {complaint}"
    );
}
