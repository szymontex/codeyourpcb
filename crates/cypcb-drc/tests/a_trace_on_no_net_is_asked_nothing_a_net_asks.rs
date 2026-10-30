//! A trace on no net is asked nothing that a net asks.
//!
//! `cargo test -p cypcb-drc --test a_trace_on_no_net_is_asked_nothing_a_net_asks`
//!
//! A net can state a width, a current, a neck, an impedance, and be one half
//! of a differential pair. Those are statements about the copper on that net.
//! Copper on no net - KiCad's net 0 - used to be read as the first net the
//! board interned, so every statement the first net made was held against it.
//!
//! Each case here puts the first net's statement and a trace that would break
//! it on a board twice: once with the trace on the net, where the rule has to
//! say so, and once with the trace on no net, where it has to say nothing.
//! The first half is what keeps the second from passing because the rule
//! cannot see anything.

use cypcb_core::{Nm, Point};
use cypcb_drc::rules::{
    DiffPairSkewRule, ImpedanceRule, MinTraceWidthRule, NeckDownRule, TraceCurrentRule,
};
use cypcb_drc::{DesignRules, DrcRule};
use cypcb_parser::ast::{DiffPairDef, Identifier};
use cypcb_parser::Span;
use cypcb_world::components::trace::{Trace, TraceNeck, TraceSegment, TraceSource};
use cypcb_world::components::Layer;
use cypcb_world::registry::NetConstraints;
use cypcb_world::{BoardWorld, NetId};

/// A board whose first net, `SIG`, states everything a net can state.
fn board() -> (BoardWorld, NetId) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
    let sig = world.intern_net("SIG");
    world.set_net_constraints(
        sig,
        NetConstraints {
            width: Some(Nm::from_mm(0.5)),
            current_ma: Some(1000.0),
            impedance_ohms_x100: Some(5000),
            neck: Some(TraceNeck {
                width: Nm::from_mm(0.05),
                length: Nm::from_mm(1.0),
            }),
            ..NetConstraints::default()
        },
    );
    (world, sig)
}

/// A 10mm trace 0.2mm wide, on `net` or on none. On a net it carries the
/// component the importer and the router give it; on none it has no component.
fn trace(world: &mut BoardWorld, net: Option<NetId>, y_mm: f64) {
    let trace = Trace {
        segments: vec![TraceSegment::new(
            Point::from_mm(10.0, y_mm),
            Point::from_mm(20.0, y_mm),
        )],
        width: Nm::from_mm(0.2),
        layer: Layer::TopCopper,
        net_id: net,
        locked: false,
        source: TraceSource::Manual,
    };
    match net {
        Some(net) => world.spawn_entity((trace, net)),
        None => world.spawn_entity((trace,)),
    };
}

fn rows(rule: &dyn DrcRule, world: &mut BoardWorld) -> usize {
    rule.check(world, &DesignRules::jlcpcb_2layer()).len()
}

/// The rule's rows with the trace on the net, and with it on none.
fn both_ways(rule: &dyn DrcRule) -> (usize, usize) {
    let (mut on_a_net, sig) = board();
    trace(&mut on_a_net, Some(sig), 10.0);
    let (mut on_none, _) = board();
    trace(&mut on_none, None, 10.0);
    (rows(rule, &mut on_a_net), rows(rule, &mut on_none))
}

#[test]
fn the_width_a_net_states_is_not_held_against_copper_on_no_net() {
    let (on_net, on_none) = both_ways(&MinTraceWidthRule);
    assert!(on_net >= 1, "the rule has to see the trace on SIG");
    assert_eq!(on_none, 0);
}

#[test]
fn the_current_a_net_carries_is_not_held_against_copper_on_no_net() {
    let (on_net, on_none) = both_ways(&TraceCurrentRule);
    assert!(on_net >= 1, "the rule has to see the trace on SIG");
    assert_eq!(on_none, 0);
}

#[test]
fn the_neck_a_net_states_is_not_measured_on_copper_on_no_net() {
    let (on_net, on_none) = both_ways(&NeckDownRule);
    assert!(on_net >= 1, "the rule has to see the trace on SIG");
    assert_eq!(on_none, 0);
}

#[test]
fn the_impedance_a_net_wants_is_not_asked_of_copper_on_no_net() {
    // No stackup on this board, so the rule says it cannot measure SIG.
    let (on_net, on_none) = both_ways(&ImpedanceRule);
    assert!(on_net >= 1, "the rule has to see the trace on SIG");
    assert_eq!(on_none, 0);
}

#[test]
fn the_impedance_a_stack_delivers_is_not_asked_of_copper_on_no_net() {
    // With a stack the rule measures instead of saying it cannot: a 0.2mm
    // trace on the first net, which asks for 90 ohm, is far off it.
    let with_stack = |net: Option<fn(&mut BoardWorld) -> Option<NetId>>| {
        let (mut world, sig) = board();
        world.set_stackup(cypcb_fixtures::a_stack_that_answers_on_every_layer());
        world.set_net_constraints(
            sig,
            NetConstraints {
                impedance_ohms_x100: Some(9000),
                ..NetConstraints::default()
            },
        );
        let net = net.map_or(Some(sig), |f| f(&mut world));
        trace(&mut world, net, 10.0);
        rows(&ImpedanceRule, &mut world)
    };
    assert!(
        with_stack(None) >= 1,
        "the rule has to see the trace on SIG"
    );
    assert_eq!(with_stack(Some(|_| None)), 0);
}

#[test]
fn copper_on_no_net_is_not_a_leg_of_a_pair() {
    let pair = |world: &mut BoardWorld| {
        world.intern_net("OTHER");
        world.set_diff_pairs(vec![DiffPairDef {
            name: Identifier::new("USB", Span::point(0)),
            positive: Identifier::new("SIG", Span::point(0)),
            negative: Identifier::new("OTHER", Span::point(0)),
            match_lengths: false,
            span: Span::point(0),
        }]);
    };

    // Control: 10mm on each half is no skew; 10mm more on SIG is.
    let (mut world, sig) = board();
    let other = NetId::new(1);
    pair(&mut world);
    trace(&mut world, Some(sig), 10.0);
    trace(&mut world, Some(other), 12.0);
    assert_eq!(rows(&DiffPairSkewRule, &mut world), 0);
    trace(&mut world, Some(sig), 14.0);
    assert_eq!(rows(&DiffPairSkewRule, &mut world), 1);

    // The same 10mm more, on no net, is not a leg of the pair.
    let (mut world, sig) = board();
    pair(&mut world);
    trace(&mut world, Some(sig), 10.0);
    trace(&mut world, Some(other), 12.0);
    trace(&mut world, None, 14.0);
    assert_eq!(rows(&DiffPairSkewRule, &mut world), 0);
}
