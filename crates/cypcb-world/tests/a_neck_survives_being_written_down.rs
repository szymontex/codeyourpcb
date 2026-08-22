//! A trace written down and read back has to be the same trace.
//!
//! `cargo test -p cypcb-world --test a_neck_survives_being_written_down`
//!
//! `neck 0.8mm for 4mm` reached the model, was drawn onto the copper, and then
//! disappeared the moment the board was written out: `board_as_dsl` emitted
//! the layer, the width, the path and the lock, and not the neck. The file
//! that came out of `cypcb route -o` still had the split vertex the neck put
//! there, so it *looked* right - but on reload the thin stretch is drawn from
//! the declaration, and a file with the geometry and no declaration reloads as
//! uniform copper at the full width.
//!
//! That is the shape this crate's `which_trace_does_not_survive_being_written
//! _down` was written for, one field later.

use cypcb_core::Nm;
use cypcb_world::components::trace::Trace;
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn load(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);
    world
}

const NECKED: &str = "version 1\n\n\
     board t {\n    size 40mm x 20mm\n    layers 2\n}\n\n\
     component R1 resistor \"0402\" {\n    value 10kohm\n    at 10mm, 10mm\n}\n\n\
     component R2 resistor \"0402\" {\n    value 10kohm\n    at 30mm, 10mm\n}\n\n\
     net SIG {\n    R1.2\n    R2.1\n}\n\n\
     trace SIG {\n    from R1.2\n    to R2.1\n    layer Top\n    width 2mm\n    \
     neck 0.8mm for 4mm\n}\n";

fn only_trace(world: &mut BoardWorld) -> Trace {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Trace>();
    let traces: Vec<Trace> = query.iter(ecs).cloned().collect();
    assert_eq!(traces.len(), 1, "one trace was written");
    traces.into_iter().next().expect("the one trace")
}

#[test]
fn the_written_file_states_the_neck() {
    let mut world = load(NECKED);
    let text = board_as_dsl(&mut world);
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("neck "))
        .unwrap_or_else(|| panic!("no neck line in:\n{text}"));
    // Six decimals is what every other dimension in this writer uses, and a
    // reader that accepts `width 2.000000mm` accepts this the same way.
    assert_eq!(line.trim(), "neck 0.800000mm for 4.000000mm");
}

#[test]
fn reading_it_back_gives_the_same_thin_copper() {
    let mut world = load(NECKED);
    let before = only_trace(&mut world).necked_length();
    // 4mm at each of the two pads the trace runs between.
    assert_eq!(before, Nm::from_mm(8.0), "the source board is necked");

    let text = board_as_dsl(&mut world);
    let mut reloaded = load(&text);

    assert_eq!(
        only_trace(&mut reloaded).necked_length(),
        before,
        "the board that came back has to run thin for as far as the one that \
         went out; it does not, so the file lost the neck"
    );
}

#[test]
fn a_trace_with_no_neck_gains_none() {
    // The common case, and the one a stray `neck` line would break: a design
    // that says nothing about necking must not come back with a declaration
    // it never made.
    let plain = NECKED.replace("    neck 0.8mm for 4mm\n", "");
    let mut world = load(&plain);
    let text = board_as_dsl(&mut world);
    assert!(
        !text.contains("neck"),
        "nothing declared a neck; the file says:\n{text}"
    );

    let mut reloaded = load(&text);
    assert_eq!(only_trace(&mut reloaded).necked_length(), Nm::ZERO);
}
