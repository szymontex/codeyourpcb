//! What a ground plane is worth, measured on the board that has one.
//!
//! `cargo test -p cypcb-autoroute --test a_plane_connects_the_pins_it_covers`
//!
//! `plane_board.kicad_pcb` is the first benchmark fixture with a pour, added
//! because every routing number this project publishes came from boards
//! without one. A plane changes what the router is solving: without it, GND is
//! a net like any other and every ground pin needs a trace; with it, those
//! pins are connected the moment the pour is filled, and the plane becomes
//! copper the signal nets have to respect.
//!
//! This measures the difference rather than asserting it. The same board is
//! checked twice - once as the file describes it, once with the pour taken
//! out - and the drop in unconnected pins is the plane's work.

use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{DrcRule, UnroutedPinRule};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_world::components::zone::Zone;
use cypcb_world::BoardWorld;

use std::path::{Path, PathBuf};

fn fixture_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

fn unreached_pins(world: &mut BoardWorld) -> usize {
    UnroutedPinRule
        .check(world, &DesignRules::jlcpcb_2layer())
        .len()
}

#[test]
fn the_pour_reaches_the_pins_that_sit_in_it() {
    let parsed =
        parse_kicad_pcb(&fixture_path("plane_board.kicad_pcb")).expect("the fixture parses");
    let mut world = parsed.world;
    world.rebuild_spatial_index_from_library(&parsed.library);

    assert_eq!(
        world.zones().len(),
        1,
        "the fixture is here for its pour and the importer did not carry one"
    );

    let with_plane = unreached_pins(&mut world);

    // The same board with the pour taken out, which is what every other
    // fixture is.
    let entities: Vec<_> = world.zones().iter().map(|(entity, _)| *entity).collect();
    for entity in entities {
        world.ecs_mut().entity_mut(entity).remove::<Zone>();
    }
    world.rebuild_spatial_index_from_library(&parsed.library);
    let without_plane = unreached_pins(&mut world);

    assert!(
        with_plane < without_plane,
        "the plane connected nothing: {with_plane} pins unreached with it, \
         {without_plane} without. A pour that connects no pin is a rectangle \
         the router avoids for no reason."
    );

    // Four, not the eleven ground pins the fixture carries - and the four are
    // the right ones.
    //
    // The pour is on B.Cu. A through-hole pin spans the stack, so it touches
    // the plane where it passes through: J1 has two ground pins, J2 and J3 one
    // each. The other seven ground pads belong to surface-mount parts on
    // F.Cu, and copper on the top layer does not touch copper on the bottom
    // one - each needs a stitching via, which is the router's job and not the
    // plane's.
    //
    // This test was first written expecting eleven, which would have meant a
    // model where a bottom-side plane silently connects top-side pads. That
    // model would route a board nobody can build.
    assert_eq!(
        without_plane - with_plane,
        4,
        "the plane is on B.Cu and reaches the four through-hole ground pins; \
         it reached {} instead",
        without_plane - with_plane
    );
}

#[test]
fn the_plane_is_on_the_layer_the_file_put_it_on() {
    // A plane on the wrong layer is worse than none: it connects pins that are
    // not on it and blocks a layer that is free.
    let parsed =
        parse_kicad_pcb(&fixture_path("plane_board.kicad_pcb")).expect("the fixture parses");
    let mut world = parsed.world;

    let zones = world.zones();
    let (_, zone) = &zones[0];
    assert_eq!(
        zone.layer_mask, 0b10,
        "the file says B.Cu and nothing else, got {:#b}",
        zone.layer_mask
    );
    assert!(zone.net.is_some(), "a pour with no net connects nothing");

    // Inset 1mm from a 50 x 38mm board. Copper to the board edge is its own
    // violation, and a fixture that fails edge clearance forty times measures
    // edge clearance rather than routing.
    assert_eq!(zone.bounds.min.x.to_mm(), 1.0);
    assert_eq!(zone.bounds.min.y.to_mm(), 1.0);
    assert_eq!(zone.bounds.max.x.to_mm(), 49.0);
    assert_eq!(zone.bounds.max.y.to_mm(), 37.0);
}
