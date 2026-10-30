//! The outline files carry the cutouts of the board.
//!
//! `cargo test -p cypcb-export --test the_outline_files_carry_the_cutouts`
//!
//! The Gerber Edge_Cuts file and the IPC-2581 Profile were written from the
//! outline ring alone, so a board with a window in it shipped as a board
//! without one and the fab did not mill it. KiCad plots every Edge.Cuts shape
//! as it is, a cutout as a contour of its own (plot_board_layers.cpp:310-318,
//! PlotLayer :54-61), and writes the Profile as one Polygon with a Cutout for
//! each hole (pcb_io_ipc2581.cpp:2741-2762 generateProfile, :2650-2672
//! addPolygonCutouts).

use cypcb_core::{Nm, Point};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::outline::export_outline;
use cypcb_export::ipc2581::{export_ipc2581, HouseTolerances};
use cypcb_export::stamp::Stamp;
use cypcb_world::components::{BoardCutouts, BoardOutline};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

const FORMAT: CoordinateFormat = CoordinateFormat::FORMAT_MM_2_6;

fn at(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn window() -> Vec<Point> {
    vec![
        at(15.0, 10.0),
        at(25.0, 10.0),
        at(25.0, 20.0),
        at(15.0, 20.0),
    ]
}

fn board(outline: bool, cutouts: Vec<Vec<Point>>) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("w".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let entity = world.board_entity().unwrap();
    if outline {
        let ring = BoardOutline::new(vec![
            at(0.0, 0.0),
            at(40.0, 0.0),
            at(40.0, 30.0),
            at(0.0, 30.0),
        ])
        .unwrap();
        world.ecs_mut().entity_mut(entity).insert(ring);
    }
    if let Some(cutouts) = BoardCutouts::new(cutouts) {
        world.ecs_mut().entity_mut(entity).insert(cutouts);
    }
    world
}

fn gerber(world: &BoardWorld) -> String {
    export_outline(world, &FORMAT, Stamp::UNIX_EPOCH).unwrap()
}

fn ipc(world: &mut BoardWorld) -> String {
    export_ipc2581(
        world,
        &FootprintLibrary::new(),
        HouseTolerances::default(),
        Stamp::UNIX_EPOCH,
    )
    .0
}

/// The drawing commands of a Gerber file, after the aperture definitions.
fn draws(gerber: &str) -> Vec<&str> {
    gerber
        .lines()
        .filter(|l| l.starts_with('X') && (l.ends_with("D01*") || l.ends_with("D02*")))
        .collect()
}

#[test]
fn the_gerber_edge_draws_each_cutout_as_a_contour_of_its_own() {
    let world = board(true, vec![window()]);
    let file = gerber(&world);
    let drawn = draws(&file);
    assert_eq!(
        drawn,
        [
            // The rim, closed.
            "X0Y0D02*",
            "X40000000Y0D01*",
            "X40000000Y30000000D01*",
            "X0Y30000000D01*",
            "X0Y0D01*",
            // The window: a fresh move, then its sides, closed.
            "X15000000Y10000000D02*",
            "X25000000Y10000000D01*",
            "X25000000Y20000000D01*",
            "X15000000Y20000000D01*",
            "X15000000Y10000000D01*",
        ]
    );
}

#[test]
fn a_board_drawn_as_a_rectangle_gets_its_rectangle_and_the_cutout() {
    let world = board(false, vec![window()]);
    let file = gerber(&world);
    let drawn = draws(&file);
    assert_eq!(drawn.len(), 10);
    assert_eq!(drawn[0], "X0Y0D02*");
    assert_eq!(drawn[2], "X40000000Y30000000D01*");
    assert_eq!(drawn[5], "X15000000Y10000000D02*");
}

#[test]
fn a_board_without_a_cutout_writes_the_file_it_always_did() {
    let world = board(true, Vec::new());
    assert_eq!(draws(&gerber(&world)).len(), 5);
    let plain = board(false, Vec::new());
    assert_eq!(
        gerber(&plain),
        gerber(&world),
        "the same rectangle, the same bytes"
    );
}

#[test]
fn the_ipc_profile_is_one_polygon_and_a_cutout_for_each_hole() {
    let mut world = board(
        true,
        vec![window(), vec![at(30.0, 5.0), at(35.0, 5.0), at(32.0, 9.0)]],
    );
    let xml = ipc(&mut world);
    let profile = &xml[xml.find("<Profile>").unwrap()..xml.find("</Profile>").unwrap()];
    assert_eq!(profile.matches("<Polygon>").count(), 1);
    assert_eq!(profile.matches("<Cutout>").count(), 2);
    // The Cutouts come after the Polygon, inside the Profile.
    assert!(profile.find("</Polygon>").unwrap() < profile.find("<Cutout>").unwrap());

    let first = &profile[profile.find("<Cutout>").unwrap()..profile.find("</Cutout>").unwrap()];
    let points: Vec<&str> = first
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(
        points,
        [
            "<PolyBegin x=\"15.000\" y=\"10.000\"/>",
            "<PolyStepSegment x=\"25.000\" y=\"10.000\"/>",
            "<PolyStepSegment x=\"25.000\" y=\"20.000\"/>",
            "<PolyStepSegment x=\"15.000\" y=\"20.000\"/>",
            "<PolyStepSegment x=\"15.000\" y=\"10.000\"/>",
        ]
    );
}

#[test]
fn a_board_without_a_cutout_has_no_cutout_in_its_profile() {
    let mut world = board(true, Vec::new());
    let xml = ipc(&mut world);
    assert!(!xml.contains("<Cutout>"));
    let profile = &xml[xml.find("<Profile>").unwrap()..xml.find("</Profile>").unwrap()];
    assert_eq!(profile.matches("<PolyStepSegment").count(), 4);
}
