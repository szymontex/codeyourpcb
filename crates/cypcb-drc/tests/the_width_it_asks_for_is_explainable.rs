//! A width IPC-2221 asks for depends on numbers nobody was told.
//!
//! `cargo test -p cypcb-drc --test the_width_it_asks_for_is_explainable`
//!
//! `TraceCurrentRule` reported `IPC-2221 wants 1.367mm on an outer layer`, and
//! that figure is only true for a particular copper thickness and a particular
//! temperature rise - 1oz and 10C, the calculator's defaults, which the
//! message never mentioned and the fab table was never asked about.
//!
//! `cypcb-calc` has taken both since it was written: 729 lines of IPC-2221
//! with a builder for copper weight, temperature rise and ambient temperature,
//! and one entry point in use that takes none of them. The rule reads the
//! fab's copper now and says what it assumed.
//!
//! Every preset states 1.0oz, which is what the default was, so this moves no
//! number - the point is that the number can be explained.

use cypcb_core::Nm;
use cypcb_drc::{run_drc, DesignRules, Preset, PresetRules};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::Layer;
use cypcb_world::BoardWorld;

/// A board with one trace on a net that declares a current.
fn board(width: Nm, current_ma: f64) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);

    let net = world.intern_net("VBUS");
    world.set_net_constraints(
        net,
        cypcb_world::registry::NetConstraints {
            current_ma: Some(current_ma),
            ..Default::default()
        },
    );

    let trace = Trace {
        segments: vec![TraceSegment::new(
            cypcb_core::Point::from_mm(5.0, 10.0),
            cypcb_core::Point::from_mm(25.0, 10.0),
        )],
        width,
        layer: Layer::TopCopper,
        net_id: net,
        locked: false,
        source: TraceSource::Manual,
    };
    world.spawn_entity((trace, net));
    world
}

fn message(rules: &DesignRules) -> String {
    message_at(3000.0, rules)
}

fn message_at(current_ma: f64, rules: &DesignRules) -> String {
    let mut world = board(Nm::from_mm(0.3), current_ma);
    let result = run_drc(&mut world, rules);
    result
        .violations
        .iter()
        .find(|v| v.kind == cypcb_drc::ViolationKind::TraceCurrent)
        .map(|v| v.message.clone())
        .unwrap_or_else(|| "no trace-current violation at all".to_string())
}

#[test]
fn the_message_says_what_the_width_assumes() {
    let rules = Preset::JlcpcbStandard2Layer.rules();
    let said = message(&rules);

    assert!(
        said.contains("1.0oz copper"),
        "the copper thickness the number assumes has to be in it: {said}"
    );
    assert!(
        said.contains("10C rise"),
        "and the temperature rise: {said}"
    );
}

#[test]
fn thicker_copper_asks_for_a_narrower_trace() {
    // The reason the thickness belongs in the message: it changes the answer.
    // No fab in the table states 2oz today, so this is the calculator being
    // asked directly - it is what a preset stating 2oz would produce.
    let mut heavy = Preset::JlcpcbStandard2Layer.rules();
    heavy.copper_weight_oz_x10 = 20;

    let thin = message(&Preset::JlcpcbStandard2Layer.rules());
    let thick = message(&heavy);

    assert!(thick.contains("2.0oz copper"), "{thick}");
    assert_ne!(
        thin, thick,
        "2oz copper carries 3A in less width than 1oz, and the message says the same thing either way"
    );
}

#[test]
fn every_preset_states_one_ounce_so_no_number_moved() {
    // What makes this change safe to make: the fab table and the calculator's
    // default agree today, so reading the table changed no board's verdict.
    for preset in Preset::all() {
        assert_eq!(
            preset.rules().copper_weight_oz_x10,
            10,
            "{} states a copper weight the old default did not",
            preset.name()
        );
    }
}

#[test]
fn a_current_past_the_data_the_standard_was_fitted_to_says_so() {
    // IPC-2221's curves were derived from measurements up to about 35A, and
    // `cypcb-calc` has said so since it was written - `TraceWidthWarning` is
    // five variants with a `Display` each, and nothing outside that crate ever
    // read one. `TraceCurrentRule` took `.width` and dropped the rest, so a
    // net asking for 40A was held to a number off the end of the data with
    // nothing on the page to say the standard does not reach that far.
    let rules = Preset::JlcpcbStandard2Layer.rules();
    let said = message_at(40_000.0, &rules);

    assert!(
        said.contains("accuracy degrades"),
        "40A is past the data IPC-2221 was fitted to and the report has to say so: {said}"
    );
    // The same calculation trips a second one, and both belong to the reader:
    // 40A at 1oz wants far more than 10mm of copper, which is several traces
    // rather than one.
    assert!(
        said.contains("multiple parallel traces"),
        "a width this far past 10mm is a bus bar, not a trace: {said}"
    );
}

#[test]
fn an_ordinary_current_carries_no_note_at_all() {
    // The other half, and the one that keeps the first from being noise: 3A at
    // 1oz and a 10C rise sits inside every range the calculator checks, so the
    // message it produces is the plain one.
    let rules = Preset::JlcpcbStandard2Layer.rules();
    let said = message(&rules);

    assert!(
        !said.contains('('),
        "nothing about this board is outside the standard, so nothing is appended: {said}"
    );
}

#[test]
fn the_boards_own_stack_beats_the_fab_table() {
    // A design that states `copper 2oz` is telling the fabricator what to
    // press. The rule read the fab table and nothing else, so a board built
    // with 2oz foil was held to the table's 1oz and asked for twice the copper
    // it needs - measured on this 5A net: 2.766mm demanded against the 1.383mm
    // the stack actually calls for.
    use cypcb_world::components::{Stackup, StackupLayer, StackupLayerKind};

    let rules = Preset::JlcpcbStandard2Layer.rules();

    let mut heavy = board(Nm::from_mm(0.5), 5000.0);
    heavy.set_stackup(Stackup {
        layers: vec![
            StackupLayer::new(
                StackupLayerKind::Copper,
                Some(Nm(2 * cypcb_core::NM_PER_OZ)),
            ),
            StackupLayer::new(StackupLayerKind::Core, Some(Nm::from_mm(1.5))),
            StackupLayer::new(
                StackupLayerKind::Copper,
                Some(Nm(2 * cypcb_core::NM_PER_OZ)),
            ),
        ],
        ..Default::default()
    });

    let said = run_drc(&mut heavy, &rules)
        .violations
        .iter()
        .find(|v| v.kind == cypcb_drc::ViolationKind::TraceCurrent)
        .map(|v| v.message.clone())
        .expect("0.5mm is too narrow for 5A on any stack");

    assert!(
        said.contains("2.0oz copper"),
        "the board states its foil and the message has to use it: {said}"
    );
    assert!(
        said.contains("1.383mm"),
        "twice the copper is half the width: {said}"
    );

    // And a board that states no stack is unchanged: the table still answers.
    let mut plain = board(Nm::from_mm(0.5), 5000.0);
    let fallback = run_drc(&mut plain, &rules)
        .violations
        .iter()
        .find(|v| v.kind == cypcb_drc::ViolationKind::TraceCurrent)
        .map(|v| v.message.clone())
        .expect("the same trace is too narrow without a stack too");
    assert!(
        fallback.contains("1.0oz copper") && fallback.contains("2.766mm"),
        "with no stack to read, the fab table is still the answer: {fallback}"
    );
}

#[test]
fn a_stack_that_states_no_thickness_leaves_the_table_standing() {
    // The half that keeps the rule honest about what it knows. A stackup entry
    // may name a layer and say nothing about how thick it is, and inventing an
    // ounce figure for it would read like a measurement.
    use cypcb_world::components::{Stackup, StackupLayer, StackupLayerKind};

    let rules = Preset::JlcpcbStandard2Layer.rules();
    let mut world = board(Nm::from_mm(0.5), 5000.0);
    world.set_stackup(Stackup {
        layers: vec![
            StackupLayer::new(StackupLayerKind::Copper, None),
            StackupLayer::new(StackupLayerKind::Core, Some(Nm::from_mm(1.5))),
            StackupLayer::new(StackupLayerKind::Copper, None),
        ],
        ..Default::default()
    });

    let said = run_drc(&mut world, &rules)
        .violations
        .iter()
        .find(|v| v.kind == cypcb_drc::ViolationKind::TraceCurrent)
        .map(|v| v.message.clone())
        .expect("still too narrow");
    assert!(
        said.contains("1.0oz copper"),
        "a stack with no foil thickness cannot answer, so the table does: {said}"
    );
}
