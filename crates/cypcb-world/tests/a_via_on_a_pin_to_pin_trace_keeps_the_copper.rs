//! A trace that states a via keeps its copper.
//!
//! `cargo test -p cypcb-world --test a_via_on_a_pin_to_pin_trace_keeps_the_copper`
//!
//! `trace SIG { from J1.1 via 10mm,10mm to J2.1 }` is the form the grammar
//! documents, and it produced **no trace at all**: two vias on the board and
//! nothing joining anything. A via was what chose the geometric branch of
//! `sync`, and that branch reads `path` and `via` and ignores `from` and `to`.
//!
//! `cypcb check` said what that looks like from outside - both pins on a net
//! no copper reaches - and no example used the form, so nothing caught it. The
//! unit test that describes the right answer could not fail either: it asserts
//! inside `for trace in query.iter(..)`, and with no trace the loop never ran.

use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld, Layer};

fn world_of(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    world
}

fn example() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/blind-via.cypcb");
    std::fs::read_to_string(&path).expect("the example is on disk")
}

#[test]
fn the_shipped_example_is_copper_and_two_holes() {
    let mut world = world_of(&example());

    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };
    assert_eq!(traces.len(), 1, "one trace joins the two pins");

    // Pin, via, via, pin: three segments, not one straight line with two
    // holes sitting off it.
    assert_eq!(
        traces[0].segments.len(),
        3,
        "the vias are corners of the trace, not decorations beside it"
    );
}

#[test]
fn the_via_stops_where_the_design_says_it_stops() {
    let mut world = world_of(&example());

    let vias: Vec<Via> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        query.iter(ecs).cloned().collect()
    };
    assert_eq!(vias.len(), 2, "the example places two");

    for via in &vias {
        assert_eq!(via.start_layer, Layer::TopCopper);
        assert_eq!(
            via.end_layer,
            Layer::Inner(0),
            "`layers Top to Inner1` is a blind via, and `Layer::Inner` is zero-based"
        );
    }
}

#[test]
fn a_block_that_states_only_a_via_is_a_via_and_not_an_empty_trace() {
    // What the writer emits for each via it saves. It has no endpoints to
    // join, so it must leave a hole and no copper - a trace with no segments
    // would be a trace that is not copper.
    let mut world = world_of(
        r#"version 1

board t {
    size 20mm x 20mm
    layers 2
}

net GND {
}

trace GND {
    via 10mm, 10mm drill 0.3mm
}
"#,
    );

    let traces = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).count()
    };
    let vias = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        query.iter(ecs).count()
    };
    assert_eq!(
        (traces, vias),
        (0, 1),
        "a hole, and nothing pretending to be copper"
    );
}
