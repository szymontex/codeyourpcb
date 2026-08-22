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
//! Three of the four checks are about the declaration: that it narrows, that
//! it is etchable, that it fits on the trace. The fourth is about the copper -
//! **how far the trace actually runs thin, against how far it said it would**.
//! That one needs a width per segment, which arrived on 2026-08-21; before it
//! the honest position was that the declaration is well formed, not the board.
//!
//! The language draws the neck as of 2026-08-22: `sync_ast_to_world` narrows
//! the last `neck.length` of the run, splitting the segment the boundary falls
//! inside. Before that a design could state a neck and the copper it produced
//! ran at one width end to end, so the fourth check had nothing to read on a
//! board written here.
//!
//! None of this decides whether the neck is thermally safe. That needs a
//! current and a temperature rise this model does not carry.

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

/// A trace whose copper runs thin for `thin_mm`, carrying a stated neck.
///
/// Built directly rather than through the language, because the language has
/// no way to say which part of a trace is the thin part - the fourth check is
/// about copper the router or the KiCad reader produced.
fn drawn_neck(thin_mm: f64, declared_mm: f64) -> BoardWorld {
    use cypcb_core::Point;
    use cypcb_world::components::trace::{Trace, TraceNeck, TraceSegment, TraceSource};
    use cypcb_world::components::Layer;

    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(20.0)), 2);
    let net = world.intern_net("SIG");

    world.ecs_mut().spawn((
        Trace {
            layer: Layer::TopCopper,
            width: Nm::from_mm(2.0),
            segments: vec![
                TraceSegment::new(Point::from_mm(4.0, 10.0), Point::from_mm(14.0, 10.0)),
                TraceSegment::new_with_width(
                    Point::from_mm(14.0, 10.0),
                    Point::from_mm(14.0 + thin_mm, 10.0),
                    Nm::from_mm(0.8),
                ),
            ],
            net_id: net,
            locked: false,
            source: TraceSource::Manual,
        },
        net,
        TraceNeck {
            width: Nm::from_mm(0.8),
            length: Nm::from_mm(declared_mm),
        },
    ));
    world
}

#[test]
fn copper_that_runs_thin_no_further_than_it_said_is_accepted() {
    let mut world = drawn_neck(4.0, 4.0);
    assert_eq!(complaints(&mut world), Vec::<String>::new());
}

#[test]
fn copper_that_runs_thin_further_than_it_said_is_reported() {
    // 6mm of 0.8mm copper under a declaration allowing 4mm. Three checks pass
    // - it narrows, it is etchable, it fits on the trace - and the fourth is
    // the only one that can see the difference.
    let mut world = drawn_neck(6.0, 4.0);
    let complaints = complaints(&mut world);
    assert_eq!(complaints.len(), 1, "got {complaints:?}");
    assert!(
        complaints[0].contains("runs thin for 6mm") && complaints[0].contains("allows 4mm"),
        "the message has to name both lengths; got {complaints:?}"
    );
}

#[test]
fn a_declared_neck_with_no_thin_copper_is_not_reported() {
    // Copper that carries a neck declaration and no thin segment: a KiCad
    // board whose tracks are uniform, opened against a design that states a
    // neck. There is nothing to measure, and zero thin millimetres must not
    // read as a trace that overran its declaration.
    let mut world = drawn_neck(0.0, 4.0);
    assert_eq!(complaints(&mut world), Vec::<String>::new());
}

#[test]
fn the_language_draws_the_neck_it_declares() {
    use cypcb_world::components::trace::Trace;

    let mut world = board("2mm", Some("neck 0.8mm for 4mm"));
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };

    assert_eq!(traces.len(), 1, "one trace was written");
    let trace = &traces[0];
    assert_eq!(
        trace.necked_length(),
        Nm::from_mm(4.0),
        "`neck 0.8mm for 4mm` has to be 4mm of 0.8mm copper, not a note beside \
         a trace that runs 2mm the whole way"
    );
    assert!(
        trace.segments.len() >= 2,
        "the run had to be cut where the width changes; it has {} segment(s)",
        trace.segments.len()
    );
    assert_eq!(
        trace.width_at(0),
        Nm::from_mm(2.0),
        "the first stretch carries the current at the stated width"
    );
    assert_eq!(
        trace.width_at(trace.segments.len() - 1),
        Nm::from_mm(0.8),
        "the far end is the necked end: the thin copper goes into the pad the \
         trace arrives at"
    );
    assert_eq!(complaints(&mut world), Vec::<String>::new());
}

#[test]
fn a_neck_no_narrower_than_its_trace_is_not_drawn() {
    // The declaration is a fault and `NeckDownRule` reports it. Drawing it as
    // geometry would turn a reported fault into copper that hides it - the
    // trace would carry a segment stating the same width it already has.
    use cypcb_world::components::trace::Trace;

    let mut world = board("2mm", Some("neck 2mm for 4mm"));
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };
    assert!(
        traces[0].segments.iter().all(|s| s.width.is_none()),
        "nothing should have been drawn"
    );
    assert_eq!(complaints(&mut world).len(), 1, "and the fault is reported");
}

/// A net whose copper reaches two pads through two chains, necked at each end.
///
/// One `Trace` per net and layer, and a net with more than two pads branches -
/// so this is the ordinary shape of a routed power net, not a corner case.
fn two_necked_branches(thin_mm: f64, declared_mm: f64) -> BoardWorld {
    use cypcb_core::Point;
    use cypcb_world::components::trace::{Trace, TraceNeck, TraceSegment, TraceSource};
    use cypcb_world::components::Layer;

    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(20.0)), 2);
    let net = world.intern_net("SIG");

    let mut segments = Vec::new();
    for y in [4.0, 14.0] {
        segments.push(TraceSegment::new(
            Point::from_mm(4.0, y),
            Point::from_mm(14.0, y),
        ));
        segments.push(TraceSegment::new_with_width(
            Point::from_mm(14.0, y),
            Point::from_mm(14.0 + thin_mm, y),
            Nm::from_mm(0.8),
        ));
    }

    world.ecs_mut().spawn((
        Trace {
            layer: Layer::TopCopper,
            width: Nm::from_mm(2.0),
            segments,
            net_id: net,
            locked: false,
            source: TraceSource::Manual,
        },
        net,
        TraceNeck {
            width: Nm::from_mm(0.8),
            length: Nm::from_mm(declared_mm),
        },
    ));
    world
}

#[test]
fn two_branches_necking_into_two_pads_do_not_add_up() {
    // 4mm at each of two pads, under a declaration allowing 4mm. The sum is
    // 8mm and the board is correct: `neck 0.8mm for 4mm` bounds one approach,
    // which is what the grammar means by "on the way into a pad". Measuring
    // the sum reported this board as overrunning by double.
    let mut world = two_necked_branches(4.0, 4.0);
    assert_eq!(complaints(&mut world), Vec::<String>::new());
}

#[test]
fn a_branch_that_overruns_is_still_caught() {
    // The same two-chain shape with 6mm at each pad. Per-approach is not a way
    // of never reporting anything: one stretch of 6mm against 4mm is a fault
    // whether or not there is a second stretch beside it.
    let mut world = two_necked_branches(6.0, 4.0);
    let complaints = complaints(&mut world);
    assert_eq!(complaints.len(), 1, "got {complaints:?}");
    assert!(
        complaints[0].contains("runs thin for 6mm in one stretch"),
        "the message has to say it is one stretch; got {complaints:?}"
    );
}

/// One chain that necks at both of its own ends: thin, wide, thin.
///
/// The shape a `from A to B` trace has when both pads are tight, and the one
/// that separates "longest unbroken stretch" from "sum of thin segments"
/// within a single run. A first draft of these tests had two chains only, and
/// a mutation deleting the reset between stretches survived it.
fn necked_at_both_ends(thin_mm: f64, declared_mm: f64) -> BoardWorld {
    use cypcb_core::Point;
    use cypcb_world::components::trace::{Trace, TraceNeck, TraceSegment, TraceSource};
    use cypcb_world::components::Layer;

    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(20.0)), 2);
    let net = world.intern_net("SIG");

    let a = 4.0;
    let b = a + thin_mm;
    let c = b + 10.0;
    let d = c + thin_mm;

    world.ecs_mut().spawn((
        Trace {
            layer: Layer::TopCopper,
            width: Nm::from_mm(2.0),
            segments: vec![
                TraceSegment::new_with_width(
                    Point::from_mm(a, 10.0),
                    Point::from_mm(b, 10.0),
                    Nm::from_mm(0.8),
                ),
                TraceSegment::new(Point::from_mm(b, 10.0), Point::from_mm(c, 10.0)),
                TraceSegment::new_with_width(
                    Point::from_mm(c, 10.0),
                    Point::from_mm(d, 10.0),
                    Nm::from_mm(0.8),
                ),
            ],
            net_id: net,
            locked: false,
            source: TraceSource::Manual,
        },
        net,
        TraceNeck {
            width: Nm::from_mm(0.8),
            length: Nm::from_mm(declared_mm),
        },
    ));
    world
}

#[test]
fn two_necks_on_one_chain_do_not_add_up_either() {
    // 3mm into each pad with 10mm of full-width copper between them, under a
    // declaration allowing 4mm. Two approaches, neither over. Summing the thin
    // segments gives 6mm and reports a board that is correct; the stretch has
    // to end where the copper widens.
    let mut world = necked_at_both_ends(3.0, 4.0);
    assert_eq!(complaints(&mut world), Vec::<String>::new());
}

#[test]
fn one_of_two_necks_on_a_chain_overrunning_is_caught() {
    let mut world = necked_at_both_ends(6.0, 4.0);
    let complaints = complaints(&mut world);
    assert_eq!(complaints.len(), 1, "got {complaints:?}");
    assert!(
        complaints[0].contains("runs thin for 6mm in one stretch"),
        "got {complaints:?}"
    );
}
