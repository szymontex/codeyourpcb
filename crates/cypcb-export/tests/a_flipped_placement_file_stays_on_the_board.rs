//! An assembly house that counts y from the top still gets this board.
//!
//! `cargo test -p cypcb-export --test a_flipped_placement_file_stays_on_the_board`
//!
//! `CplConfig::with_flipped_y` is public, has a constructor, has tests, and
//! wrote a file no machine could use: the coordinate was **negated** rather
//! than flipped, under a comment saying it would need the board height to do
//! it properly. A part 10mm up a 20mm board came out at -10mm - off the board,
//! on the wrong side of the origin, every part of every design.
//!
//! Its own tests passed throughout, because they asserted that the flag was
//! set rather than what the flag did. Nothing in the exporter passes a config,
//! so no board shipped wrong; a public constructor that produces a broken file
//! is still a trap laid for the first caller.
//!
//! The flip is about the board's far edge now, which is the only thing "y from
//! the top" can mean.

use cypcb_core::Nm;
use cypcb_export::cpl::{export_cpl, CplConfig};
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation, Value};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

const BOARD_HEIGHT_MM: f64 = 20.0;

/// A board 30mm x 20mm with one part 10mm up from the bottom edge.
fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "placed".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(BOARD_HEIGHT_MM)),
        2,
    );
    let library = FootprintLibrary::new();

    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(12.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );

    (world, library)
}

/// The y coordinate of R1 in a placement file.
fn y_of_r1(csv: &str) -> f64 {
    let row = csv
        .lines()
        .find(|line| line.starts_with("R1,"))
        .unwrap_or_else(|| panic!("R1 is not in the file:\n{csv}"));
    let field = row.split(',').nth(2).expect("a y column");
    field
        .trim_end_matches("mm")
        .parse()
        .unwrap_or_else(|err| panic!("y is not a number in {row}: {err}"))
}

fn placement(config: Option<&CplConfig>) -> String {
    let (mut world, library) = board();
    export_cpl(&mut world, &library, config).expect("the placement file exports")
}

#[test]
fn a_part_keeps_its_place_when_nothing_is_flipped() {
    // The control, and what every export writes today: the exporter passes no
    // config at all.
    assert_eq!(y_of_r1(&placement(None)), 10.0);
}

#[test]
fn a_flipped_file_measures_from_the_other_edge() {
    // 20mm board, part 10mm up: 10mm down from the top. On this board the two
    // happen to agree, which is why the next test uses a part that is not in
    // the middle.
    let flipped = CplConfig::with_flipped_y();

    assert_eq!(y_of_r1(&placement(Some(&flipped))), BOARD_HEIGHT_MM - 10.0);
}

#[test]
fn every_part_stays_on_the_board() {
    // The defect in one line: the old code wrote -10.0 here.
    let mut world = BoardWorld::new();
    world.set_board(
        "placed".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(BOARD_HEIGHT_MM)),
        2,
    );
    let library = FootprintLibrary::new();
    for (index, y) in [2.0f64, 7.5, 18.0].iter().enumerate() {
        world.spawn_component(
            RefDes::new(format!("R{}", index + 1)),
            Value::new("10k"),
            Position::from_mm(5.0 + index as f64 * 5.0, *y),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );
    }

    let flipped = CplConfig::with_flipped_y();
    let csv = export_cpl(&mut world, &library, Some(&flipped)).expect("it exports");

    let ys: Vec<f64> = csv
        .lines()
        .filter(|line| line.starts_with('R'))
        .filter_map(|line| line.split(',').nth(2))
        .filter_map(|field| field.trim_end_matches("mm").parse().ok())
        .collect();

    assert_eq!(ys.len(), 3, "three parts: {csv}");
    for y in &ys {
        assert!(
            *y >= 0.0 && *y <= BOARD_HEIGHT_MM,
            "{y}mm is off a {BOARD_HEIGHT_MM}mm board:\n{csv}"
        );
    }
    // And it is a flip rather than a shuffle: the part nearest the bottom edge
    // is now the one nearest the top.
    assert_eq!(ys[0], BOARD_HEIGHT_MM - 2.0);
    assert_eq!(ys[2], BOARD_HEIGHT_MM - 18.0);
}

#[test]
fn a_design_with_no_board_is_left_alone() {
    // There is nothing to flip about, and negating the coordinate would be
    // the old defect wearing a different hat.
    let mut world = BoardWorld::new();
    let library = FootprintLibrary::new();
    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(12.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );

    let flipped = CplConfig::with_flipped_y();
    let csv = export_cpl(&mut world, &library, Some(&flipped)).expect("it exports");

    assert_eq!(y_of_r1(&csv), 10.0);
}
