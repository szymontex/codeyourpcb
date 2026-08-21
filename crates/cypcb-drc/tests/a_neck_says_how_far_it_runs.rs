//! A stated neck is measured, not taken on trust.
//!
//! `cargo test -p cypcb-drc --test a_neck_says_how_far_it_runs`
//!
//! The owner's request, 2026-08-11: on a mains board `netclass Mains [current
//! 10A]` gives a trace several millimetres wide, correctly, and a 2.54mm pad
//! pitch has nowhere to put it. Every other EDA lets the last stretch before a
//! pad run thin, because a short length of copper does not have time to heat.
//! `neck 0.8mm for 4mm` is how this language says it.
//!
//! This is the level that makes the statement legal and checkable. It does not
//! decide whether the neck is thermally safe: a trace in this model carries one
//! width and the necked stretch is not in the segments yet, so what is checked
//! is that the declaration describes a neck at all.

use cypcb_core::Nm;
use cypcb_drc::{run_drc, DesignRules};
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

/// A board with one trace of `width`, optionally carrying `neck`.
fn board(width: &str, neck: Option<&str>) -> BoardWorld {
    let neck = neck.map(|n| format!("    {n}\n")).unwrap_or_default();
    load(&format!(
        "version 1\n\n\
         board t {{\n    size 40mm x 20mm\n    layers 2\n}}\n\n\
         component R1 resistor \"0402\" {{\n    value 10kohm\n    at 10mm, 10mm\n}}\n\n\
         component R2 resistor \"0402\" {{\n    value 10kohm\n    at 30mm, 10mm\n}}\n\n\
         net SIG {{\n    R1.2\n    R2.1\n}}\n\n\
         trace SIG {{\n    from R1.2\n    to R2.1\n    layer Top\n    width {width}\n{neck}}}\n"
    ))
}

/// Every neck-down message, and nothing from the other rules.
fn complaints(world: &mut BoardWorld) -> Vec<String> {
    run_drc(world, &DesignRules::default())
        .violations
        .into_iter()
        .filter(|violation| violation.kind.to_string() == "neck-down")
        .map(|violation| violation.message)
        .collect()
}

#[test]
fn a_neck_narrower_than_its_trace_is_accepted() {
    // The case the owner asked for: wide copper for the current, thin copper
    // for the last few millimetres into the pad.
    let mut world = board("2mm", Some("neck 0.8mm for 4mm"));
    assert_eq!(complaints(&mut world), Vec::<String>::new());
}

#[test]
fn a_trace_with_no_neck_is_not_measured_for_one() {
    let mut world = board("2mm", None);
    assert_eq!(complaints(&mut world), Vec::<String>::new());
}

#[test]
fn a_neck_that_does_not_narrow_is_a_second_width() {
    // Equal is not narrower, and wider is the same mistake written larger.
    for neck in ["neck 2mm for 4mm", "neck 3mm for 4mm"] {
        let mut world = board("2mm", Some(neck));
        let said = complaints(&mut world);
        assert_eq!(said.len(), 1, "{neck}: {said:?}");
        assert!(
            said[0].contains("is not narrower than"),
            "{neck}: {}",
            said[0]
        );
    }
}

#[test]
fn a_neck_under_what_the_fab_will_etch_is_refused() {
    // A neck is a licence to go thin, not a licence to go thinner than the
    // house can make. The default table is JLCPCB two-layer at 0.127mm.
    let mut world = board("2mm", Some("neck 0.05mm for 4mm"));
    let said = complaints(&mut world);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("will etch"), "{}", said[0]);
    assert!(said[0].contains("0.127mm"), "{}", said[0]);
}

#[test]
fn a_neck_longer_than_its_trace_is_the_whole_trace() {
    // The trace runs from 10.5mm to 29.5mm, so 19mm of copper. A neck allowed
    // to run 25mm leaves nothing at the wide width, which is the declaration
    // saying the opposite of what it means.
    let mut world = board("2mm", Some("neck 0.8mm for 25mm"));
    let said = complaints(&mut world);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("the whole trace is the neck"),
        "{}",
        said[0]
    );
}

#[test]
fn the_two_faults_are_reported_separately() {
    // A neck can be both too wide and too long, and a reader fixing one
    // should not have to run the checker again to find the other.
    let mut world = board("2mm", Some("neck 2mm for 25mm"));
    let said = complaints(&mut world);
    assert_eq!(said.len(), 2, "{said:?}");
}

#[test]
fn the_length_is_compulsory() {
    // A width with no length is a second width. The whole point of stating a
    // neck is that its length is bounded.
    let parsed = cypcb_parser::parse(
        "version 1\n\nboard t {\n    size 40mm x 20mm\n    layers 2\n}\n\n\
         trace SIG {\n    width 2mm\n    neck 0.8mm\n}\n",
    );
    assert!(!parsed.errors.is_empty(), "`neck 0.8mm` was accepted alone");
}

#[test]
fn the_neck_reaches_the_model_as_written() {
    use cypcb_world::components::trace::TraceNeck;
    let mut world = board("2mm", Some("neck 0.8mm for 4mm"));
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&TraceNeck>();
    let necks: Vec<TraceNeck> = query.iter(ecs).copied().collect();
    assert_eq!(
        necks,
        vec![TraceNeck {
            width: Nm::from_mm(0.8),
            length: Nm::from_mm(4.0),
        }]
    );
}
