//! A blind or buried via, written in the language and read back.
//!
//! Three parts of the tool read a via's span - the viewer draws it, the drill
//! export gives it a file, the hole rule measures against it - and until this
//! landed the DSL had no way to state one. A span could only arrive from a
//! KiCad import, and a design saved from the viewer lost it silently.

use cypcb_core::Nm;
use cypcb_world::components::trace::Via;
use cypcb_world::components::Layer;
use cypcb_world::dsl::traces_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

// The net has to exist before a trace block can name it.
const BOARD: &str = "board t {\n    size 30mm x 20mm\n    layers 4\n}\n\nnet GND {\n}\n\n";

fn load(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    world
}

fn vias(world: &mut BoardWorld) -> Vec<Via> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Via>();
    query.iter(ecs).copied().collect()
}

#[test]
fn a_via_with_no_stated_pair_goes_through() {
    let mut world = load(&format!(
        "{BOARD}trace GND {{\n    via 10mm,10mm drill 0.3mm\n}}\n"
    ));
    let vias = vias(&mut world);

    assert_eq!(vias.len(), 1);
    assert_eq!(vias[0].start_layer, Layer::TopCopper);
    assert_eq!(vias[0].end_layer, Layer::BottomCopper);
}

#[test]
fn a_via_that_states_a_pair_keeps_it() {
    let mut world = load(&format!(
        "{BOARD}trace GND {{\n    via 10mm,10mm drill 0.2mm layers Inner1 to Inner2\n}}\n"
    ));
    let vias = vias(&mut world);

    assert_eq!(vias.len(), 1);
    assert_eq!(vias[0].start_layer, Layer::Inner(0));
    assert_eq!(vias[0].end_layer, Layer::Inner(1));
}

#[test]
fn the_span_survives_a_round_trip_through_the_writer() {
    // A design saved from the viewer and reloaded has to be the same board.
    let source = format!(
        "{BOARD}trace GND {{\n    via 10mm,10mm drill 0.2mm layers Top to Inner1\n}}\n"
    );
    let mut world = load(&source);

    let written = traces_as_dsl(&mut world);
    assert!(
        written.contains("layers Top to Inner1"),
        "the writer dropped the span:\n{written}"
    );

    let mut reloaded = load(&format!("{BOARD}{written}"));
    let vias = vias(&mut reloaded);
    assert_eq!(vias.len(), 1);
    assert_eq!(vias[0].start_layer, Layer::TopCopper);
    assert_eq!(vias[0].end_layer, Layer::Inner(0));
}

#[test]
fn a_through_via_is_written_without_the_noise() {
    // Every file would carry `layers Top to Bottom` on every via otherwise,
    // and its absence already means through.
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 4);
    let net = world.intern_net("GND");
    let via = Via::new(cypcb_core::Point::from_mm(10.0, 10.0), net);
    world.ecs_mut().spawn((via, net));

    let written = traces_as_dsl(&mut world);
    assert!(written.contains("via 10.000000mm,10.000000mm"), "{written}");
    assert!(!written.contains("layers"), "{written}");
}
