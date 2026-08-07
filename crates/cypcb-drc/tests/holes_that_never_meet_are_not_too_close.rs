//! Two holes only have to keep their distance if the same drill makes them.
//!
//! A via buried between In1 and In2 and one between In3 and the bottom are
//! made in separate passes on separate sub-stacks, before the board is pressed
//! together. Measuring them against each other reports a fault the board does
//! not have - and a designer who is told about faults that are not there stops
//! reading the list.

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
use cypcb_world::components::trace::Via;
use cypcb_world::components::Layer;
use cypcb_world::BoardWorld;

/// A four-layer board with two vias almost on top of each other.
fn board_with_two_vias(first: (Layer, Layer), second: (Layer, Layer)) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 4);
    let net = world.intern_net("GND");

    let mut a = Via::new(Point::from_mm(10.0, 10.0), net);
    a.start_layer = first.0;
    a.end_layer = first.1;

    // 0.05mm apart edge to edge: unmanufacturable if one drill makes both.
    let mut b = Via::new(Point::from_mm(10.35, 10.0), net);
    b.start_layer = second.0;
    b.end_layer = second.1;

    world.ecs_mut().spawn((a, net));
    world.ecs_mut().spawn((b, net));
    world
}

fn hole_faults(world: &mut BoardWorld) -> usize {
    run_drc(world, &DesignRules::jlcpcb_2layer())
        .violations
        .iter()
        .filter(|violation| violation.kind == ViolationKind::HoleToHole)
        .count()
}

#[test]
fn two_holes_from_the_same_drill_pass_are_reported() {
    let mut world = board_with_two_vias(
        (Layer::TopCopper, Layer::BottomCopper),
        (Layer::TopCopper, Layer::BottomCopper),
    );
    assert_eq!(
        hole_faults(&mut world),
        1,
        "two through holes 0.05mm apart cannot be drilled"
    );
}

#[test]
fn two_holes_on_different_sub_stacks_are_not() {
    let mut world = board_with_two_vias(
        (Layer::TopCopper, Layer::Inner(0)),
        (Layer::Inner(1), Layer::BottomCopper),
    );
    assert_eq!(
        hole_faults(&mut world),
        0,
        "these are made in different passes and never meet"
    );
}

#[test]
fn a_blind_via_under_a_through_hole_is_still_reported() {
    // The direction that matters: they do share a pass, and the blind one is
    // drilled into a board the through hole already goes through.
    let mut world = board_with_two_vias(
        (Layer::TopCopper, Layer::BottomCopper),
        (Layer::TopCopper, Layer::Inner(0)),
    );
    assert_eq!(hole_faults(&mut world), 1);
}
