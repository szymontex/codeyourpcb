//! A board exported to KiCad still shows where its parts are.
//!
//! `cargo test -p cypcb-kicad --test the_board_arrives_with_its_legend`
//!
//! The Gerber writer has printed a legend for every part since the silkscreen
//! was written: a footprint's own artwork when it has any, its courtyard
//! outline when it does not - which is what tells a person where a part sits
//! when they look at a bare board.
//!
//! The KiCad writer printed none of it. Measured on one example board, the
//! same design through both writers: **38 stroked segments** in
//! `pour-island-F_SilkS.gbr` against **0** `fp_line` in the `.kicad_pcb`.
//! Three parts, each an outline in the fabrication files and nothing on
//! screen in KiCad.
//!
//! Not checked by a round trip, unlike the pours and the part sides: this
//! project's KiCad reader does not read `fp_line` at all, so there is nothing
//! to read it back with. Saying so is better than a test that pretends.

use cypcb_core::{Nm, Point};
use cypcb_kicad::board_writer::write_board;
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation, Value};
use cypcb_world::footprint::{Footprint, FootprintLibrary, SilkShape};
use cypcb_world::BoardWorld;

/// A board with one 0402, whose built-in footprint carries no artwork.
fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "legend".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        2,
    );
    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );
    world
}

/// The same board, with a footprint that states real artwork.
fn board_with_artwork() -> BoardWorld {
    let mut world = board();
    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(Footprint {
        name: "drawn".to_string(),
        silk: vec![
            SilkShape::Segment {
                start: Point::from_mm(-1.0, -0.6),
                end: Point::from_mm(1.0, -0.6),
                width: Nm::from_mm(0.15),
            },
            SilkShape::Circle {
                centre: Point::from_mm(-1.2, 0.0),
                radius: Nm::from_mm(0.2),
                width: Nm::from_mm(0.15),
            },
        ],
        ..base
    });
    world.set_footprints(library);

    let entity = world
        .components()
        .first()
        .map(|(entity, _, _)| *entity)
        .expect("the board has a part");
    world
        .ecs_mut()
        .entity_mut(entity)
        .insert(FootprintRef::new("drawn"));
    world
}

#[test]
fn a_part_with_no_artwork_gets_its_courtyard_drawn() {
    let text = write_board(&mut board(), "cypcb");
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("fp_line"))
        .collect();

    assert_eq!(lines.len(), 4, "a closed rectangle is four lines:\n{text}");
    for line in &lines {
        assert!(line.contains("\"F.SilkS\""), "{line}");
    }
}

#[test]
fn the_outline_closes() {
    // Four lines that do not meet are four scratches. Every corner has to be
    // the end of one line and the start of another.
    let text = write_board(&mut board(), "cypcb");
    let ends: Vec<(String, String)> = text
        .lines()
        .filter(|line| line.contains("fp_line"))
        .map(|line| {
            let start = line
                .split("(start ")
                .nth(1)
                .and_then(|rest| rest.split(')').next())
                .unwrap_or_default()
                .to_string();
            let end = line
                .split("(end ")
                .nth(1)
                .and_then(|rest| rest.split(')').next())
                .unwrap_or_default()
                .to_string();
            (start, end)
        })
        .collect();

    assert_eq!(ends.len(), 4);
    for pair in ends.windows(2) {
        assert_eq!(pair[0].1, pair[1].0, "the outline breaks: {ends:?}");
    }
    assert_eq!(ends[3].1, ends[0].0, "and it has to come back round");
}

#[test]
fn a_footprint_with_artwork_prints_its_artwork_instead() {
    // Drawing the courtyard as well would put a box on the board the
    // footprint never asked for - the same rule the Gerber writer follows.
    let text = write_board(&mut board_with_artwork(), "cypcb");

    assert_eq!(
        text.matches("fp_line").count(),
        1,
        "one segment of artwork, and no courtyard:\n{text}"
    );
    assert_eq!(text.matches("fp_circle").count(), 1, "{text}");
    assert!(text.contains("(width 0.15)"), "the stated stroke:\n{text}");
}

#[test]
fn a_board_with_no_parts_draws_no_legend() {
    let mut world = BoardWorld::new();
    world.set_board(
        "bare".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );

    let text = write_board(&mut world, "cypcb");
    assert!(!text.contains("fp_line"), "{text}");
}
