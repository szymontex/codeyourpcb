//! A corner sharper than a right angle is reported, and a right angle is not.
//!
//! `cargo test -p cypcb-drc --test copper_that_meets_itself_too_sharply`
//!
//! Every case here is one net on one layer, drawn as a polyline, because that
//! is what the rule looks at: the angle between the two arms of a junction.
//! The numbers are exact - 180, 135, 90, 45 and 0 degrees - so nothing in this
//! file depends on how a cosine rounds.

use cypcb_core::{Nm, Point};
use cypcb_drc::rules::AcuteAngleRule;
use cypcb_drc::{DesignRules, DrcRule, ViolationKind};
use cypcb_world::components::trace::{Curve, Trace, TraceSegment};
use cypcb_world::components::Layer;
use cypcb_world::BoardWorld;

/// One run of copper: the net it belongs to, the layer it is on, and the
/// points it turns at.
type Polyline<'a> = (&'a str, Layer, &'a [(f64, f64)]);

/// A board carrying one trace per polyline, on the net and layer given.
fn board(polylines: &[Polyline]) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);

    for (net_name, layer, points) in polylines {
        let net = world.intern_net(net_name);
        let mut trace = Trace::new(net);
        trace.layer = *layer;
        trace.width = Nm::from_mm(0.25);
        for pair in points.windows(2) {
            trace.add_segment(TraceSegment::new(
                Point::from_mm(pair[0].0, pair[0].1),
                Point::from_mm(pair[1].0, pair[1].1),
            ));
        }
        world.ecs_mut().spawn((trace, net));
    }
    world
}

/// Every message the rule produces, corners and unmeasured junctions alike.
fn reported(world: &mut BoardWorld) -> Vec<String> {
    AcuteAngleRule
        .check(world, &DesignRules::jlcpcb_2layer())
        .into_iter()
        .inspect(|violation| assert_eq!(violation.kind, ViolationKind::AcidTrap))
        .map(|violation| violation.message)
        .collect()
}

/// The corners it reports, without the junctions it says it did not measure.
fn corners(world: &mut BoardWorld) -> Vec<String> {
    reported(world)
        .into_iter()
        .filter(|message| !message.contains("Not checked"))
        .collect()
}

#[test]
fn copper_that_runs_straight_or_turns_wide_is_not_a_trap() {
    // 180 degrees, then 135 - a 45 degree turn, which is how a trace is
    // supposed to change direction.
    let mut world = board(&[
        (
            "SIG",
            Layer::TopCopper,
            &[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)],
        ),
        (
            "OTHER",
            Layer::TopCopper,
            &[(0.0, 5.0), (10.0, 5.0), (20.0, 15.0)],
        ),
    ]);
    assert_eq!(corners(&mut world), Vec::<String>::new());
}

#[test]
fn a_square_corner_passes_and_that_is_the_whole_line() {
    // Exactly 90 degrees. The rule reports strictly below it, so this is the
    // case that dies the moment the comparison is written the other way.
    let mut world = board(&[(
        "SIG",
        Layer::TopCopper,
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
    )]);
    assert_eq!(corners(&mut world), Vec::<String>::new());
}

#[test]
fn a_corner_sharper_than_a_right_angle_is_reported_with_its_angle() {
    // 45 degrees: the trace leaves the corner back towards where it came from.
    let mut world = board(&[(
        "SIG",
        Layer::TopCopper,
        &[(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)],
    )]);
    let found = corners(&mut world);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("45.0 degrees"), "{found:?}");
    assert!(found[0].contains("net 'SIG'"), "{found:?}");
}

#[test]
fn copper_drawn_over_itself_is_not_called_a_wedge() {
    // Zero degrees, on one line: there is no wedge here at all, the copper is
    // laid down twice. Saying "0.0 degrees" would be true and useless, and
    // calling it a trap for etchant would be wrong.
    let mut world = board(&[(
        "SIG",
        Layer::TopCopper,
        &[(0.0, 0.0), (10.0, 0.0), (2.0, 0.0)],
    )]);
    let found = corners(&mut world);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("drawn over itself"), "{found:?}");
    assert!(!found[0].contains("traps etchant"), "{found:?}");
}

#[test]
fn a_junction_is_one_report_however_many_arms_meet_there() {
    // A fanout: three arms leaving one point, two of them acute against the
    // third. The designer moves the corner once.
    let mut world = board(&[
        ("SIG", Layer::TopCopper, &[(10.0, 0.0), (0.0, 0.0)]),
        ("SIG", Layer::TopCopper, &[(10.0, 0.0), (0.0, 10.0)]),
        ("SIG", Layer::TopCopper, &[(10.0, 0.0), (0.0, -10.0)]),
    ]);
    let found = corners(&mut world);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("45.0 degrees"), "{found:?}");
}

#[test]
fn copper_that_never_meets_has_no_corner() {
    // The same sharp shape drawn twice: once as two nets, once as two layers.
    // Neither is a junction - one is a crossing, which is `ClearanceRule`'s
    // question, and the other is a via, which is two corners on two layers.
    let mut nets = board(&[
        ("SIG", Layer::TopCopper, &[(0.0, 0.0), (10.0, 0.0)]),
        ("GND", Layer::TopCopper, &[(10.0, 0.0), (0.0, 10.0)]),
    ]);
    assert_eq!(corners(&mut nets), Vec::<String>::new());

    let mut layers = board(&[
        ("SIG", Layer::TopCopper, &[(0.0, 0.0), (10.0, 0.0)]),
        ("SIG", Layer::BottomCopper, &[(10.0, 0.0), (0.0, 10.0)]),
    ]);
    assert_eq!(corners(&mut layers), Vec::<String>::new());
}

#[test]
fn a_t_junction_is_reported_as_not_measured_once_for_the_net() {
    // One trace across, one trace ending in the middle of it. The angle is
    // real and this rule does not compute it, so it says so - once, however
    // many times the net does it.
    // The same net does it four times over, on two layers: twice on top and
    // twice on the bottom. One sentence covers all four, which is the point -
    // a per-layer report would say it twice and a per-junction report four
    // times.
    let mut world = board(&[
        ("SIG", Layer::TopCopper, &[(0.0, 0.0), (20.0, 0.0)]),
        ("SIG", Layer::TopCopper, &[(10.0, 0.0), (10.0, 10.0)]),
        ("SIG", Layer::TopCopper, &[(5.0, 0.0), (5.0, 10.0)]),
        ("SIG", Layer::BottomCopper, &[(0.0, 20.0), (20.0, 20.0)]),
        ("SIG", Layer::BottomCopper, &[(10.0, 20.0), (10.0, 30.0)]),
        ("SIG", Layer::BottomCopper, &[(5.0, 20.0), (5.0, 30.0)]),
    ]);
    let all = reported(&mut world);
    let unmeasured: Vec<&String> = all
        .iter()
        .filter(|message| message.contains("Not checked"))
        .collect();
    assert_eq!(unmeasured.len(), 1, "{all:?}");
    assert!(unmeasured[0].contains("middle of its own trace"), "{all:?}");
}

#[test]
fn a_curve_is_left_alone() {
    // A curved trace is stored as the chords of its arc, so its interior
    // junctions are angles nobody drew. This one is drawn sharp on purpose.
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
    let net = world.intern_net("SIG");
    let mut trace = Trace::new(net);
    trace.layer = Layer::TopCopper;
    trace.width = Nm::from_mm(0.25);
    trace.add_segment(TraceSegment::new(
        Point::from_mm(0.0, 0.0),
        Point::from_mm(10.0, 0.0),
    ));
    trace.add_segment(TraceSegment::new(
        Point::from_mm(10.0, 0.0),
        Point::from_mm(0.0, 10.0),
    ));
    world.ecs_mut().spawn((
        trace,
        net,
        Curve {
            centre: Point::from_mm(5.0, 5.0),
            sweep_millideg: 90_000,
        },
    ));

    assert_eq!(corners(&mut world), Vec::<String>::new());
}
