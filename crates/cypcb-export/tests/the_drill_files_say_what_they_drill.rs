//! A drill file has to say which holes it is and which layers they join.
//!
//! `cargo test -p cypcb-export --test the_drill_files_say_what_they_drill`
//!
//! The Excellon files said nothing about themselves: `M48`, a comment naming
//! the generator, and coordinates. Every other file this exporter writes has
//! carried a Gerber X2 `TF.FileFunction` for as long as it has existed, and
//! the drill files - the ones that decide where the holes go and whether they
//! are plated - carried none.
//!
//! That showed up the moment the job file was written. It describes the file
//! set for a fabricator by reading what each file states about itself, so a
//! drill file that states nothing is a board with no holes: eleven Gerbers
//! described and the drill file mentioned nowhere.
//!
//! The span matters as much as the plating. A blind or buried via joins two
//! layers that are not the outside of the board, and a drill file with no
//! stated span means "through everything" to every fabricator.

use cypcb_core::{Nm, Point};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::{export_excellon, export_excellon_span, DrillType};
use cypcb_world::components::trace::Via;
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// A four-layer board with a through via and one buried between the inner pair.
fn board(layers: u8) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "drilled".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        layers,
    );
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

/// What a drill file states about itself.
fn stated_function(drill: &str) -> String {
    drill
        .lines()
        .find_map(|line| line.split("TF.FileFunction,").nth(1))
        .map(str::trim)
        .map(str::to_string)
        .unwrap_or_else(|| panic!("the file states no function:\n{drill}"))
}

fn through(world: &mut BoardWorld, library: &FootprintLibrary, kind: DrillType) -> String {
    export_excellon(world, library, &CoordinateFormat::FORMAT_MM_2_6, Some(kind))
        .expect("the drill file exports")
}

#[test]
fn the_through_file_states_the_whole_board() {
    let (mut world, library) = board(4);
    let drill = through(&mut world, &library, DrillType::Plated);

    assert_eq!(stated_function(&drill), "Plated,1,4,PTH");
}

#[test]
fn a_two_layer_board_spans_two_layers() {
    // The span is the board's, not a constant: this is the same layer numbering
    // the copper files use, through one shared function.
    let (mut world, library) = board(2);
    let drill = through(&mut world, &library, DrillType::Plated);

    assert_eq!(stated_function(&drill), "Plated,1,2,PTH");
}

#[test]
fn the_holes_that_must_not_be_plated_say_so() {
    // A mounting hole in the plated file comes back plated - narrower than the
    // screw it was drilled for, and connected to whatever copper it passes.
    let (mut world, library) = board(4);
    let drill = through(&mut world, &library, DrillType::NonPlated);

    assert_eq!(stated_function(&drill), "NonPlated,1,4,NPTH");
}

#[test]
fn a_buried_via_says_which_two_layers_it_joins() {
    // Inner(0) and Inner(1) are Gerber layers 2 and 3, and neither is an
    // outside face, so these holes are buried rather than blind.
    let (mut world, library) = board(4);
    let drill = export_excellon_span(
        &mut world,
        &library,
        &CoordinateFormat::FORMAT_MM_2_6,
        Some(DrillType::Plated),
        (Layer::Inner(0), Layer::Inner(1)),
    )
    .expect("the span exports");

    assert_eq!(stated_function(&drill), "Plated,2,3,Buried");
}

#[test]
fn a_via_that_reaches_a_face_is_blind_rather_than_buried() {
    let (mut world, library) = board(4);
    let drill = export_excellon_span(
        &mut world,
        &library,
        &CoordinateFormat::FORMAT_MM_2_6,
        Some(DrillType::Plated),
        (Layer::TopCopper, Layer::Inner(0)),
    )
    .expect("the span exports");

    assert_eq!(stated_function(&drill), "Plated,1,2,Blind");
}

#[test]
fn a_file_with_no_holes_in_it_still_says_what_it_is() {
    // The exporter writes a plated file whether or not the board has plated
    // holes, and an empty file that describes nothing is one a fabricator
    // cannot file against the job.
    let mut world = BoardWorld::new();
    world.set_board(
        "bare".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    let library = FootprintLibrary::new();

    let drill = through(&mut world, &library, DrillType::Plated);
    assert!(
        !drill.lines().any(|line| line.starts_with('X')),
        "this board has no holes"
    );
    assert_eq!(stated_function(&drill), "Plated,1,2,PTH");
}
