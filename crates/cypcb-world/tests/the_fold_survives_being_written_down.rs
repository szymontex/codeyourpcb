//! The fold survives being written down.
//!
//! `cargo test -p cypcb-world --test the_fold_survives_being_written_down`
//!
//! `radius 2mm` is what the checker measures a flexible region against, and a
//! save that dropped it would hand back a file whose fold nothing can check -
//! the defect the stackup had before the writer learned to carry it, and the
//! reason every word this language gains gets a round trip of its own.

use cypcb_world::components::{BendRadius, Zone};
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

const FOLDED: &str = r#"version 1

board wearable {
    size 60mm x 16mm
    layers 2
}

flex bend {
    bounds 22mm, 0mm to 38mm, 16mm
    layer all
    radius 2mm
}
"#;

fn load(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);
    world
}

fn radius_of(world: &mut BoardWorld) -> Option<cypcb_core::Nm> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&Zone, &BendRadius)>();
    query
        .iter(ecs)
        .find(|(zone, _)| zone.is_flex())
        .map(|(_, radius)| radius.0)
}

#[test]
fn the_model_holds_the_fold_the_design_states() {
    let mut world = load(FOLDED);
    assert_eq!(
        radius_of(&mut world),
        Some(cypcb_core::Nm::from_mm(2.0)),
        "the region says how tightly it is folded"
    );
}

#[test]
fn the_writer_gives_it_back_and_the_reader_takes_it_again() {
    let mut world = load(FOLDED);
    let written = board_as_dsl(&mut world);
    // In the writer's own form - six decimals of a millimetre - so the line is
    // found and read rather than matched against a spelling this test would
    // have to keep in step with.
    let line = written
        .lines()
        .find(|line| line.trim_start().starts_with("radius "))
        .unwrap_or_else(|| panic!("the fold comes back out of the writer:\n{written}"));
    assert!(
        line.contains("2.000000mm") || line.contains("2mm"),
        "and it is the fold the design stated: {line}"
    );

    let mut again = load(&format!("version 1\n\n{written}"));
    assert_eq!(
        radius_of(&mut again),
        Some(cypcb_core::Nm::from_mm(2.0)),
        "and reads back as the same fold"
    );
}

#[test]
fn a_region_that_says_nothing_writes_nothing() {
    // The half that keeps the writer honest: a file that gained `radius 0mm`
    // on its first save would state a fold nobody asked for, and the checker
    // would then refuse the board.
    let mut world = load(&FOLDED.replace("    radius 2mm\n", ""));
    let written = board_as_dsl(&mut world);
    assert!(
        !written.contains("radius"),
        "nothing is invented for a region that did not say:\n{written}"
    );
}
