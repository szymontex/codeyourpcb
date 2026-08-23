//! A via is a hole somebody has to drill, and not every span is drillable.
//!
//! `cargo test -p cypcb-drc --test a_via_is_a_hole_somebody_drills`
//!
//! `blind_vias_allowed` and `buried_vias_allowed` have been in every fab table
//! since the tables were written, and `DesignRules::from_constraints` dropped
//! them before they reached any rule - so a flag every house sets checked
//! nothing. The same gap `castellated_holes_allowed` had, one hole over.

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, DesignRules, Preset, PresetRules, ViolationKind};
use cypcb_world::components::trace::Via;
use cypcb_world::components::{DrillPair, Layer, Stackup};
use cypcb_world::BoardWorld;

/// A board with one via between these two layers.
fn board(copper_layers: u8, start: Layer, end: Layer, pairs: &[(Layer, Layer)]) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "t".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        copper_layers,
    );
    if !pairs.is_empty() {
        let stackup = Stackup {
            drill_pairs: pairs
                .iter()
                .map(|(start, end)| DrillPair {
                    start: *start,
                    end: *end,
                })
                .collect(),
            ..Stackup::default()
        };
        assert!(world.set_stackup(stackup), "the board takes a stackup");
    }
    let net_id = world.intern_net("SIG");
    world.ecs_mut().spawn((
        Via {
            position: Point::from_mm(15.0, 10.0),
            drill: Nm::from_mm(0.3),
            outer_diameter: Nm::from_mm(0.6),
            net_id,
            start_layer: start,
            end_layer: end,
            locked: false,
        },
        net_id,
    ));
    world
}

/// What the span rule says, under these rules.
fn faults(world: &mut BoardWorld, rules: &DesignRules) -> Vec<String> {
    run_drc(world, rules)
        .violations
        .into_iter()
        .filter(|violation| violation.kind == ViolationKind::ViaSpan)
        .map(|violation| violation.message)
        .collect()
}

fn jlcpcb_4layer() -> DesignRules {
    Preset::JlcpcbStandard4Layer.rules()
}

#[test]
fn a_through_via_is_not_reported() {
    // The control. Every ordinary board is full of these, and a rule that
    // fires on them is a rule nobody keeps.
    let mut world = board(4, Layer::TopCopper, Layer::BottomCopper, &[]);
    assert_eq!(faults(&mut world, &jlcpcb_4layer()), Vec::<String>::new());
}

#[test]
fn a_blind_via_where_the_house_drills_none_is_reported() {
    let mut world = board(4, Layer::TopCopper, Layer::Inner(1), &[]);
    let said = faults(&mut world, &jlcpcb_4layer());
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("is a blind via"), "{}", said[0]);
    assert!(
        said[0].contains("Top to Inner2"),
        "the message names the span the design wrote: {}",
        said[0]
    );
}

#[test]
fn a_buried_via_is_told_apart_from_a_blind_one() {
    // Neither face: a different hole, a different price, and a different flag
    // in the table.
    let mut world = board(4, Layer::Inner(0), Layer::Inner(1), &[]);
    let said = faults(&mut world, &jlcpcb_4layer());
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("is a buried via"), "{}", said[0]);
}

#[test]
fn a_house_that_drills_them_is_not_reported() {
    // The other half of the same question. No preset ships `true` today, so
    // the rules are built by hand here.
    let mut rules = jlcpcb_4layer();
    rules.blind_vias_allowed = true;
    rules.buried_vias_allowed = true;

    let mut blind = board(4, Layer::TopCopper, Layer::Inner(1), &[]);
    assert_eq!(faults(&mut blind, &rules), Vec::<String>::new());

    let mut buried = board(4, Layer::Inner(0), Layer::Inner(1), &[]);
    assert_eq!(faults(&mut buried, &rules), Vec::<String>::new());
}

#[test]
fn a_span_the_build_does_not_drill_is_reported_even_where_the_house_could() {
    // The design's own list. `drill Top to Inner1` says which cycle drills
    // what, and a via outside the list is a hole this build does not make
    // whatever the house is capable of.
    let mut rules = jlcpcb_4layer();
    rules.blind_vias_allowed = true;

    let mut world = board(
        4,
        Layer::TopCopper,
        Layer::Inner(1),
        &[
            (Layer::TopCopper, Layer::BottomCopper),
            (Layer::TopCopper, Layer::Inner(0)),
        ],
    );
    let said = faults(&mut world, &rules);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("not a span this build drills"),
        "{}",
        said[0]
    );
    assert!(
        said[0].contains("Top to Inner1"),
        "the message lists what the stackup does state: {}",
        said[0]
    );
}

#[test]
fn a_span_on_the_list_is_not_reported() {
    let mut rules = jlcpcb_4layer();
    rules.blind_vias_allowed = true;

    let mut world = board(
        4,
        Layer::TopCopper,
        Layer::Inner(0),
        &[(Layer::TopCopper, Layer::Inner(0))],
    );
    assert_eq!(faults(&mut world, &rules), Vec::<String>::new());
}

#[test]
fn the_list_is_read_either_way_round() {
    // A hole from the top layer to the first inner one is the same hole
    // whichever end the design wrote first.
    let mut rules = jlcpcb_4layer();
    rules.blind_vias_allowed = true;

    let mut world = board(
        4,
        Layer::Inner(0),
        Layer::TopCopper,
        &[(Layer::TopCopper, Layer::Inner(0))],
    );
    assert_eq!(faults(&mut world, &rules), Vec::<String>::new());
}

#[test]
fn a_two_layer_board_is_left_alone() {
    // One span exists and it is the through hole. Asking would report every
    // via on every ordinary board.
    let mut world = board(2, Layer::TopCopper, Layer::BottomCopper, &[]);
    assert_eq!(
        faults(&mut world, &Preset::JlcpcbStandard2Layer.rules()),
        Vec::<String>::new()
    );
}

#[test]
fn both_flags_reach_the_rules_from_the_table_they_are_written_in() {
    // The half a rule test cannot see: `from_constraints` is the only place
    // these cross from a fab table into what a rule reads, so hard-coding
    // them there leaves every test above green while nothing a table says can
    // reach a board again.
    let mut constraints = cypcb_rules::DesignConstraints::default();
    assert!(!constraints.blind_vias_allowed, "the premise");
    assert!(!constraints.buried_vias_allowed, "the premise");

    constraints.blind_vias_allowed = true;
    constraints.buried_vias_allowed = true;
    let rules = DesignRules::from_constraints(&constraints);
    assert!(rules.blind_vias_allowed, "a yes in the table is a yes here");
    assert!(
        rules.buried_vias_allowed,
        "a yes in the table is a yes here"
    );
}
