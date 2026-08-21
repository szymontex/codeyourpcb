//! A controlled-impedance net is measured, or it is told it was not.
//!
//! `cargo test -p cypcb-drc --test what_the_stack_delivers_against_what_the_net_asked_for`
//!
//! The last join. A net states a target, the stack says which form its layer
//! calls for, `cypcb-calc` computes, and this compares. What the tests below
//! spend most of their length on is the third outcome - the layer whose
//! surroundings the stack cannot describe - because that one looks exactly
//! like a pass and is not one.

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, DesignRules};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{Layer, Stackup, StackupLayer, StackupLayerKind};
use cypcb_world::registry::NetConstraints;
use cypcb_world::BoardWorld;

use StackupLayerKind::{Copper, Core, Prepreg};

/// kind, thickness in mm, dk in thousandths.
type Spec = (StackupLayerKind, Option<f64>, Option<u32>);

/// Four coppers with the same dielectric between every pair, so the inner
/// layers are genuinely centred and the stack can answer for all four.
const CENTRED: &[Spec] = &[
    (Copper, Some(0.035), None),
    (Prepreg, Some(0.2), Some(4_600)),
    (Copper, Some(0.0175), None),
    (Prepreg, Some(0.2), Some(4_600)),
    (Copper, Some(0.0175), None),
    (Prepreg, Some(0.2), Some(4_600)),
    (Copper, Some(0.035), None),
];

/// The ordinary build: prepreg outside, a thick core in the middle, so the
/// inner layers are not centred.
const ORDINARY: &[Spec] = &[
    (Copper, Some(0.035), None),
    (Prepreg, Some(0.2), Some(4_600)),
    (Copper, Some(0.0175), None),
    (Core, Some(1.095), Some(4_500)),
    (Copper, Some(0.0175), None),
    (Prepreg, Some(0.2), Some(4_600)),
    (Copper, Some(0.035), None),
];

fn board(stack: &[Spec], layer: Layer, width_mm: f64, target_ohms_x100: Option<u32>) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("z".to_string(), (Nm::from_mm(40.0), Nm::from_mm(20.0)), 4);
    world.set_stackup(Stackup {
        layers: stack
            .iter()
            .map(|(kind, thickness, dk)| StackupLayer {
                kind: *kind,
                name: None,
                thickness: thickness.map(Nm::from_mm),
                material: None,
                dk_x1000: *dk,
                df_x1000000: None,
            })
            .collect(),
    });

    let net_id = world.intern_net("CLK");
    if let Some(target) = target_ohms_x100 {
        world.set_net_constraints(
            net_id,
            NetConstraints {
                impedance_ohms_x100: Some(target),
                ..Default::default()
            },
        );
    }
    world.ecs_mut().spawn((
        Trace {
            segments: vec![TraceSegment {
                start: Point::new(Nm::from_mm(5.0), Nm::from_mm(10.0)),
                end: Point::new(Nm::from_mm(35.0), Nm::from_mm(10.0)),
            }],
            width: Nm::from_mm(width_mm),
            layer,
            net_id,
            locked: false,
            source: TraceSource::Manual,
        },
        net_id,
    ));
    world
}

/// Every message this rule produced, and nothing from the other rules.
fn complaints(world: &mut BoardWorld) -> Vec<String> {
    let report = run_drc(world, &DesignRules::default());
    report
        .violations
        .into_iter()
        .filter(|violation| violation.kind.to_string() == "impedance")
        .map(|violation| violation.message)
        .collect()
}

#[test]
fn a_trace_that_hits_its_target_is_silent() {
    // 0.35mm on 0.2mm of 4.6 laminate is 47.35 ohm - 87/sqrt(6.01) times
    // ln(1.196/0.315) - and a net asking for 50 is 5.3% away, inside the ten
    // percent this rule reports outside of.
    let mut world = board(CENTRED, Layer::TopCopper, 0.35, Some(5_000));
    assert_eq!(complaints(&mut world), Vec::<String>::new());
}

#[test]
fn a_trace_that_misses_says_both_numbers_and_the_gap() {
    // The same geometry against a 90 ohm target: 47.35 against 90 is nowhere
    // near, and a differential pair asked to be 90 is the ordinary way this
    // goes wrong.
    let mut world = board(CENTRED, Layer::TopCopper, 0.35, Some(9_000));
    let said = complaints(&mut world);
    assert_eq!(said.len(), 1, "{said:?}");
    let message = &said[0];
    assert!(message.contains("asks for 90ohm"), "{message}");
    assert!(message.contains("gives 47.35ohm"), "{message}");
    assert!(message.contains("% off"), "{message}");
    // And what the number is worth, in the same breath.
    assert!(message.contains("5-7%"), "{message}");
}

#[test]
fn a_layer_the_stack_cannot_describe_is_reported_as_not_checked() {
    // The ordinary four-layer build. L2 has prepreg above and core below, so
    // it is an asymmetric stripline and no form here covers it. Saying
    // nothing would read as a pass on a controlled-impedance net.
    let mut world = board(ORDINARY, Layer::Inner(1), 0.2, Some(5_000));
    let said = complaints(&mut world);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("Not checked - not passed"), "{}", said[0]);
    assert!(said[0].contains("asks for 50ohm"), "{}", said[0]);
}

#[test]
fn a_net_that_asked_for_nothing_is_not_measured() {
    // Most nets. The rule is for the ones that stated a target.
    let mut world = board(CENTRED, Layer::TopCopper, 0.35, None);
    assert_eq!(complaints(&mut world), Vec::<String>::new());
}

#[test]
fn an_inner_layer_that_is_centred_is_measured_as_a_stripline() {
    // 0.2mm between two 0.2mm prepregs: the stripline form, and well under
    // any ordinary target, so it reports.
    let mut world = board(CENTRED, Layer::Inner(1), 0.2, Some(9_000));
    let said = complaints(&mut world);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        !said[0].contains("Not checked"),
        "a centred inner layer is answerable: {}",
        said[0]
    );
    assert!(said[0].contains("Inner(1)"), "{}", said[0]);
}

#[test]
fn the_tolerance_is_ten_percent_and_it_is_a_boundary_not_a_slope() {
    // The trace gives 47.35 ohm. A target it is 8.9% away from stays quiet and
    // one it is 13.9% away from does not. The form is quoted at 5-7%, so
    // anything tighter than ten would be reporting the equation's own error as
    // a fault on the board.
    let mut inside = board(CENTRED, Layer::TopCopper, 0.35, Some(5_200));
    assert_eq!(
        complaints(&mut inside),
        Vec::<String>::new(),
        "47.35 against 52 is 8.9% and under the bar"
    );

    let mut outside = board(CENTRED, Layer::TopCopper, 0.35, Some(5_500));
    assert_eq!(
        complaints(&mut outside).len(),
        1,
        "47.35 against 55 is 13.9% and over it"
    );
}

#[test]
fn the_bottom_layer_is_measured_against_the_dielectric_on_its_own_side() {
    // A stack whose two faces are not alike: 0.1mm under the top copper and
    // 0.3mm over the bottom. On a symmetric stack both outer layers give the
    // same answer, so a mapping that reads the bottom as the top survives
    // every other test in this file. This is the one that does not let it.
    let lopsided: &[Spec] = &[
        (Copper, Some(0.035), None),
        (Prepreg, Some(0.1), Some(4_600)),
        (Copper, Some(0.0175), None),
        (Core, Some(1.0), Some(4_500)),
        (Copper, Some(0.0175), None),
        (Prepreg, Some(0.3), Some(4_600)),
        (Copper, Some(0.035), None),
    ];

    // Top: 5.98 * 0.1 / 0.315 = 1.898, ln = 0.6410, 87/sqrt(6.01) = 35.488,
    // so 22.75 ohm.
    let mut top = board(lopsided, Layer::TopCopper, 0.35, Some(9_000));
    let said = complaints(&mut top);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("gives 22.75ohm"), "{}", said[0]);

    // Bottom: 5.98 * 0.3 / 0.315 = 5.695238, ln = 1.739630, so 61.74 ohm. A rule
    // that looked at the top's dielectric would say 22.75 here too.
    let mut bottom = board(lopsided, Layer::BottomCopper, 0.35, Some(9_000));
    let said = complaints(&mut bottom);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("gives 61.74ohm"),
        "the bottom layer has 0.3mm under it, not the top's 0.1mm: {}",
        said[0]
    );
}

#[test]
fn the_first_inner_layer_is_the_second_copper_entry() {
    // `Layer::Inner` is zero-based - the language's `Inner1` is
    // `Layer::Inner(0)` - and the stack's copper sequence is not, because its
    // first entry is the top layer. So `Inner(0)` is copper entry 1.
    //
    // On the centred stack a 0.2mm trace on the first inner layer is a
    // stripline: B = 0.4mm, T = 0.0175mm, so 0.8W + T = 0.1775, 4B/(0.67 pi *
    // 0.1775) = 4.28226, ln = 1.45448, and 60/sqrt(4.6) = 27.97516 gives
    // 40.69 ohm.
    //
    // Read as copper entry 0 it would be the **top** layer's microstrip -
    // 0.2mm over 0.2mm with the top's 0.035mm foil - which is 64.37 ohm. The
    // two are far enough apart that no rounding hides the difference.
    let mut world = board(CENTRED, Layer::Inner(0), 0.2, Some(9_000));
    let said = complaints(&mut world);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("gives 40.69ohm"),
        "the first inner layer is a stripline between both prepregs: {}",
        said[0]
    );
    assert!(
        !said[0].contains("64.37ohm"),
        "that is the top layer's answer: {}",
        said[0]
    );
}

#[test]
fn every_layer_of_the_shared_fixture_reports_a_different_impedance() {
    // The property that makes an index error visible, asserted through the
    // rule rather than through the lookup: four layers, four numbers, none
    // repeated. A rule reading the wrong layer would report a number that
    // belongs to another one, and this is the assertion that would catch it -
    // which the symmetric fixture used elsewhere in this file cannot do, and
    // did not, three times.
    let layers = [
        Layer::TopCopper,
        Layer::Inner(0),
        Layer::Inner(1),
        Layer::BottomCopper,
    ];

    let mut reported = Vec::new();
    for layer in layers {
        let mut world = BoardWorld::new();
        world.set_board("z".to_string(), (Nm::from_mm(40.0), Nm::from_mm(20.0)), 4);
        world.set_stackup(cypcb_fixtures::every_copper_layer_answers_differently());
        let net_id = world.intern_net("CLK");
        world.set_net_constraints(
            net_id,
            NetConstraints {
                // Far from anything the stack can give, so every layer reports
                // and the message carries its number.
                impedance_ohms_x100: Some(1_000),
                ..Default::default()
            },
        );
        world.ecs_mut().spawn((
            Trace {
                segments: vec![TraceSegment {
                    start: Point::new(Nm::from_mm(5.0), Nm::from_mm(10.0)),
                    end: Point::new(Nm::from_mm(35.0), Nm::from_mm(10.0)),
                }],
                width: Nm::from_mm(0.2),
                layer,
                net_id,
                locked: false,
                source: TraceSource::Manual,
            },
            net_id,
        ));

        let said = complaints(&mut world);
        assert_eq!(said.len(), 1, "{layer:?}: {said:?}");
        let ohms = said[0]
            .split("gives ")
            .nth(1)
            .and_then(|rest| rest.split("ohm").next())
            .unwrap_or_else(|| panic!("{layer:?} reported no number: {}", said[0]))
            .to_string();
        reported.push((format!("{layer:?}"), ohms));
    }

    let mut numbers: Vec<&str> = reported.iter().map(|(_, ohms)| ohms.as_str()).collect();
    numbers.sort_unstable();
    let before = numbers.len();
    numbers.dedup();
    assert_eq!(
        numbers.len(),
        before,
        "two layers reported the same impedance, so this fixture cannot catch an index error: {reported:?}"
    );
}
