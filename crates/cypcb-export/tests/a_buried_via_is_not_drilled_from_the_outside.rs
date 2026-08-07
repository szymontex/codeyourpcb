//! A blind or buried via belongs in a drill file of its own.
//!
//! An Excellon file with no stated layer pair means "through the whole board"
//! to every fabricator. Putting a via that stops at an inner layer into that
//! file has the board drilled from the outside - a board nobody can make, and
//! nothing in the file set says otherwise.

use cypcb_core::{Nm, Point};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::{export_excellon, export_excellon_span, non_through_spans, DrillType};
use cypcb_world::components::trace::Via;
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// A four-layer board with one through via and one buried between the inner
/// pair.
fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 4);

    let net = world.intern_net("GND");

    let through = Via::new(Point::from_mm(5.0, 5.0), net);

    let mut buried = Via::new(Point::from_mm(20.0, 10.0), net);
    buried.drill = Nm::from_mm(0.2);
    buried.start_layer = Layer::Inner(0);
    buried.end_layer = Layer::Inner(1);

    world.ecs_mut().spawn((through, net));
    world.ecs_mut().spawn((buried, net));

    (world, FootprintLibrary::new())
}

#[test]
fn the_through_file_carries_only_the_through_hole() {
    let (mut world, library) = board();
    let drill = export_excellon(
        &mut world,
        &library,
        &CoordinateFormat::FORMAT_MM_2_6,
        Some(DrillType::Plated),
    )
    .expect("the drill file is written");

    assert!(
        drill.contains("X5.000000Y5.000000"),
        "the through via should be drilled through:\n{drill}"
    );
    assert!(
        !drill.contains("X20.000000Y10.000000"),
        "the buried via must not be drilled from the outside:\n{drill}"
    );
}

#[test]
fn the_buried_via_gets_a_file_for_the_pair_it_joins() {
    let (mut world, library) = board();

    let spans = non_through_spans(&mut world, &library).expect("the spans are found");
    assert_eq!(
        spans,
        vec![(Layer::Inner(0), Layer::Inner(1))],
        "one pair beyond the through pair"
    );

    let drill = export_excellon_span(
        &mut world,
        &library,
        &CoordinateFormat::FORMAT_MM_2_6,
        Some(DrillType::Plated),
        (Layer::Inner(0), Layer::Inner(1)),
    )
    .expect("the pair's drill file is written");

    assert!(
        drill.contains("X20.000000Y10.000000"),
        "the buried via belongs in its own file:\n{drill}"
    );
    assert!(
        !drill.contains("X5.000000Y5.000000"),
        "and the through via does not:\n{drill}"
    );
}

#[test]
fn a_board_with_only_through_holes_asks_for_no_extra_files() {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let net = world.intern_net("GND");
    let via = Via::new(Point::from_mm(5.0, 5.0), net);
    world.ecs_mut().spawn((via, net));

    let library = FootprintLibrary::new();
    assert!(non_through_spans(&mut world, &library)
        .expect("the spans are found")
        .is_empty());
}
