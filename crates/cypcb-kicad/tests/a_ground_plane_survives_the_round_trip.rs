//! A board exported to KiCad still has its ground plane.
//!
//! `cargo test -p cypcb-kicad --test a_ground_plane_survives_the_round_trip`
//!
//! `cypcb to-kicad` wrote footprints, traces, vias, the net list and the
//! outline, and walked past every zone. Measured on the bundled example:
//! `cargo run -p cypcb-cli -- to-kicad examples/pour-island.cypcb` produced a
//! file with **0** occurrences of `zone` for a design whose whole point is a
//! ground pour. The board opened in KiCad without its ground - the same
//! silence the Gerber writer had before the pour was implemented, one file
//! over.
//!
//! This project reads zones out of KiCad boards already, and that reader's
//! fixture is the authority for the shape: a net number and name, a layer, a
//! filled polygon. So the check here is the round trip - write it, read it
//! back with our own parser, and require the plane to still be there, on the
//! right net, over the right rectangle.

use cypcb_core::{Nm, Point, Rect};
use cypcb_kicad::board_writer::write_board;
use cypcb_kicad::pcb_parser::parse_kicad_pcb_str;
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::BoardWorld;

/// A 40x30 board with a ground pour over most of it.
fn board_with_pour(layer_mask: u32) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "planed".to_string(),
        (Nm::from_mm(40.0), Nm::from_mm(30.0)),
        2,
    );
    let gnd = world.intern_net("GND");

    let bounds = Rect::new(Point::from_mm(2.0, 2.0), Point::from_mm(38.0, 28.0));
    world.spawn_entity((Zone {
        bounds,
        kind: ZoneKind::CopperPour,
        layer_mask,
        name: Some("gnd_pour".to_string()),
        net: Some(gnd),
    },));

    world
}

/// The zones a written board still holds after being read back.
fn zones_after_round_trip(world: &mut BoardWorld) -> Vec<Zone> {
    let text = write_board(world, "cypcb");
    let parsed = parse_kicad_pcb_str(&text).expect("this project can read its own output");
    let mut read_back = parsed.world;
    let ecs = read_back.ecs_mut();
    let mut query = ecs.query::<&Zone>();
    query.iter(ecs).cloned().collect()
}

#[test]
fn the_plane_is_still_there_after_a_round_trip() {
    let zones = zones_after_round_trip(&mut board_with_pour(0b01));

    assert_eq!(zones.len(), 1, "one pour went in: {zones:?}");
    assert_eq!(zones[0].kind, ZoneKind::CopperPour);
}

#[test]
fn it_comes_back_over_the_same_rectangle() {
    // A plane that survives the trip smaller than it went in is a board with
    // a ring of bare laminate nobody asked for.
    let zones = zones_after_round_trip(&mut board_with_pour(0b01));

    assert_eq!(zones[0].bounds.min, Point::from_mm(2.0, 2.0));
    assert_eq!(zones[0].bounds.max, Point::from_mm(38.0, 28.0));
}

#[test]
fn it_comes_back_on_its_own_net() {
    // A pour with no net is not a pour: it cannot be filled and the pads it
    // swallows are connected to nothing.
    let mut world = board_with_pour(0b01);
    let text = write_board(&mut world, "cypcb");
    let parsed = parse_kicad_pcb_str(&text).expect("it reads back");
    let mut read_back = parsed.world;

    let net = {
        let ecs = read_back.ecs_mut();
        let mut query = ecs.query::<&Zone>();
        query.iter(ecs).next().and_then(|zone| zone.net)
    };
    let net = net.expect("the pour kept a net");

    assert_eq!(read_back.net_name(net), Some("GND"));
}

#[test]
fn a_plane_on_both_faces_is_written_as_two() {
    // KiCad stores a pour per layer - `(layers ...)` on a zone is for rule
    // areas rather than for copper - so a design pouring both faces has to
    // produce two of them, or the back of the board loses its ground.
    let mut world = board_with_pour(0b11);
    let text = write_board(&mut world, "cypcb");

    assert_eq!(text.matches("  (zone").count(), 2, "{text}");
    assert!(text.contains("(layer \"F.Cu\")"));
    assert!(text.contains("(layer \"B.Cu\")"));

    let zones = zones_after_round_trip(&mut board_with_pour(0b11));
    assert_eq!(zones.len(), 2, "both faces came back: {zones:?}");
}

#[test]
fn a_board_with_no_zones_writes_none() {
    // The control. Every other example in this repository has no pour, and
    // none of them may grow one.
    let mut world = BoardWorld::new();
    world.set_board(
        "bare".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );

    let text = write_board(&mut world, "cypcb");
    assert!(!text.contains("(zone"), "{text}");
}
