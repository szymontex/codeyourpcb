//! Breaking the round trip on purpose, now that both halves count.
//!
//! `cargo test -p cypcb-world --test which_trace_does_not_survive_being_written_down`
//!
//! The owner has twice seen a trace vanish while wiring by hand: draw one,
//! draw the next, the first one goes. The viewer now counts the traces through
//! both halves of the journey, and the tracker's next action is this - drive
//! the shapes a hand-drawn trace can take through the writer and the reader,
//! and see which of them does not come back.
//!
//! Six shapes, chosen because each is a different way for the language to fall
//! short rather than because they are six: two traces on one net, three, one
//! per layer, one carrying a via, one whose segments do not chain, and one on
//! a net whose name needs quoting. `dsl.rs` names the via as the written
//! suspect - its outer diameter is rebuilt from twice the drill rather than
//! read - so that shape is here to be measured rather than assumed.

use std::collections::BTreeMap;

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::components::Layer;
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// How many trace entities each net carries, by name.
fn census(world: &mut BoardWorld) -> BTreeMap<String, usize> {
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };
    let mut out = BTreeMap::new();
    for trace in traces {
        let net = world
            .net_name(trace.net_id)
            .unwrap_or("unnamed")
            .to_string();
        *out.entry(net).or_insert(0) += 1;
    }
    out
}

/// Every segment on the board, as comparable integers and net names.
fn segments(world: &mut BoardWorld) -> Vec<(String, String, i64, i64, i64, i64)> {
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };
    let mut out = Vec::new();
    for trace in &traces {
        let net = world
            .net_name(trace.net_id)
            .unwrap_or("unnamed")
            .to_string();
        for segment in &trace.segments {
            out.push((
                net.clone(),
                format!("{:?}", trace.layer),
                segment.start.x.raw(),
                segment.start.y.raw(),
                segment.end.x.raw(),
                segment.end.y.raw(),
            ));
        }
    }
    out.sort();
    out
}

/// Every via, likewise, carrying the diameter `dsl.rs` says it cannot write.
fn vias(world: &mut BoardWorld) -> Vec<(String, i64, i64, i64, i64)> {
    let placed: Vec<Via> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        query.iter(ecs).copied().collect()
    };
    let mut out: Vec<_> = placed
        .into_iter()
        .map(|via| {
            let net = world.net_name(via.net_id).unwrap_or("unnamed").to_string();
            (
                net,
                via.position.x.raw(),
                via.position.y.raw(),
                via.drill.raw(),
                via.outer_diameter.raw(),
            )
        })
        .collect();
    out.sort();
    out
}

fn load(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);
    world
}

/// A bare two-layer board with two parts and one net, ready to carry copper.
fn base_board(net_name: &str) -> String {
    format!(
        r#"version 1

board t {{
    size 40mm x 20mm
    layers 2
}}

component R1 resistor "0402" {{
    value 10kohm
    at 10mm, 10mm
}}

component R2 resistor "0402" {{
    value 10kohm
    at 30mm, 10mm
}}

net {net_name} {{
    R1.2
    R2.1
}}
"#
    )
}

/// Put a trace on the board directly, so a shape the language cannot spell can
/// still be built and asked about.
fn add_trace(world: &mut BoardWorld, net: &str, layer: Layer, segments: Vec<TraceSegment>) {
    let net_id = world.intern_net(net);
    world.ecs_mut().spawn((
        Trace {
            segments,
            width: Nm::from_mm(0.25),
            layer,
            net_id,
            locked: false,
            source: TraceSource::Manual,
        },
        net_id,
    ));
}

fn run(x0: f64, y0: f64, x1: f64, y1: f64) -> TraceSegment {
    TraceSegment {
        start: Point::from_mm(x0, y0),
        end: Point::from_mm(x1, y1),
    }
}

/// Write the world out, read it back, and return the two censuses.
fn round_trip(world: &mut BoardWorld) -> (BTreeMap<String, usize>, BoardWorld, String) {
    let before = census(world);
    let text = board_as_dsl(world);
    let back = load(&text);
    (before, back, text)
}

#[test]
fn two_traces_on_one_net_both_come_back() {
    // The owner's report in its simplest form: draw one, draw the next.
    let mut world = load(&base_board("SIG"));
    add_trace(
        &mut world,
        "SIG",
        Layer::TopCopper,
        vec![run(11.0, 10.0, 20.0, 10.0)],
    );
    add_trace(
        &mut world,
        "SIG",
        Layer::TopCopper,
        vec![run(20.0, 14.0, 29.0, 14.0)],
    );

    let (before, mut back, text) = round_trip(&mut world);
    assert_eq!(
        before.get("SIG"),
        Some(&2),
        "the premise: two traces went in"
    );
    assert_eq!(
        segments(&mut back).len(),
        2,
        "both segments have to come back:\n{text}"
    );
}

#[test]
fn three_traces_on_one_net_all_come_back() {
    let mut world = load(&base_board("SIG"));
    for (index, y) in [10.0f64, 12.0, 14.0].into_iter().enumerate() {
        add_trace(
            &mut world,
            "SIG",
            Layer::TopCopper,
            vec![run(11.0 + index as f64, y, 20.0, y)],
        );
    }

    let (before, mut back, text) = round_trip(&mut world);
    assert_eq!(before.get("SIG"), Some(&3));
    assert_eq!(segments(&mut back).len(), 3, "\n{text}");
}

#[test]
fn a_trace_on_each_layer_keeps_its_layer() {
    let mut world = load(&base_board("SIG"));
    add_trace(
        &mut world,
        "SIG",
        Layer::TopCopper,
        vec![run(11.0, 10.0, 20.0, 10.0)],
    );
    add_trace(
        &mut world,
        "SIG",
        Layer::BottomCopper,
        vec![run(20.0, 10.0, 29.0, 10.0)],
    );

    let (_, mut back, text) = round_trip(&mut world);
    let layers: Vec<String> = segments(&mut back).into_iter().map(|s| s.1).collect();
    assert!(layers.contains(&"TopCopper".to_string()), "\n{text}");
    assert!(layers.contains(&"BottomCopper".to_string()), "\n{text}");
    assert_eq!(layers.len(), 2, "\n{text}");
}

#[test]
fn a_net_the_language_could_not_name_now_round_trips_quoted() {
    // This test used to be called `a_net_the_language_cannot_name_is_left_out_
    // and_said_out_loud`, and it asserted that the writer dropped such a net's
    // copper and printed a comment naming it. That was the honest answer while
    // the grammar had no quoted form: emitting a bare `VBUS+` produced a file
    // the parser rejects, which on the viewer's save path is work the user
    // cannot reopen.
    //
    // It carried a guard against its own premise - "the language accepts a
    // quoted net name now; this test and the writer both need revisiting" -
    // and that guard is what fired when `net_name` was added. `VBUS+`, `D+`
    // and `D-` are on every USB design there is, so the copper on them being
    // dropped was never a resting place.
    let quoted = base_board("\"VBUS+\"");
    let parsed = cypcb_parser::parse(&quoted);
    assert!(
        parsed.errors.is_empty(),
        "a quoted net name is legal now: {:?}",
        parsed.errors
    );

    // The board's own net is the awkward one, so it carries pins and gets a
    // declaration. A trace on a net nothing connects to is a different gap -
    // the writer emits the trace and no `net` block, and sync then reports
    // `MissingNet` - and that one predates this change and is recorded rather
    // than fixed here.
    let mut world = load(&quoted);
    add_trace(
        &mut world,
        "VBUS+",
        Layer::TopCopper,
        vec![run(11.0, 10.0, 20.0, 10.0)],
    );
    assert_eq!(census(&mut world).get("VBUS+"), Some(&1), "the premise");

    let text = board_as_dsl(&mut world);
    let written_back = cypcb_parser::parse(&text);
    assert!(
        written_back.errors.is_empty(),
        "the writer produced a file its own parser rejects:\n{text}\n{:?}",
        written_back.errors
    );
    assert!(
        text.contains("trace \"VBUS+\""),
        "a name the identifier rule refuses has to be quoted, not dropped:\n{text}"
    );
    assert!(
        !text.contains("no way to name"),
        "nothing is being left out any more, so nothing should say it is:\n{text}"
    );

    // The copper arrives, on the net it left on.
    let mut back = load(&text);
    assert_eq!(census(&mut back).get("VBUS+"), Some(&1), "\n{text}");
}

#[test]
fn segments_that_do_not_chain_all_come_back() {
    // `contiguous_runs` splits a trace wherever a segment does not begin where
    // the last one ended, so one entity becomes two blocks. Splitting is not
    // losing - what this asserts is that no segment goes missing in the split,
    // which is the failure the count alone would hide.
    let mut world = load(&base_board("SIG"));
    add_trace(
        &mut world,
        "SIG",
        Layer::TopCopper,
        vec![
            run(11.0, 10.0, 15.0, 10.0),
            // Starts somewhere else entirely: a second run.
            run(20.0, 14.0, 25.0, 14.0),
        ],
    );

    let (_, mut back, text) = round_trip(&mut world);
    let came_back = segments(&mut back);
    assert_eq!(
        came_back.len(),
        2,
        "a segment was dropped when the trace was split into runs:\n{text}"
    );
}

#[test]
fn a_via_survives_and_its_diameter_is_the_documented_casualty() {
    // The written suspect. `dsl.rs` says a via's outer diameter has no syntax
    // and is rebuilt as twice the drill on the way back in. This measures
    // that rather than trusting the comment: the via itself must survive, and
    // its diameter is expected to be replaced - so if the comment ever stops
    // being true, in either direction, this says so.
    let mut world = load(&base_board("SIG"));
    let net_id = world.intern_net("SIG");
    world.ecs_mut().spawn((
        Via {
            position: Point::from_mm(20.0, 10.0),
            drill: Nm::from_mm(0.3),
            // Deliberately not twice the drill.
            outer_diameter: Nm::from_mm(0.9),
            net_id,
            start_layer: Layer::TopCopper,
            end_layer: Layer::BottomCopper,
            locked: false,
        },
        net_id,
    ));

    let before = vias(&mut world);
    assert_eq!(before.len(), 1, "the premise: one via went in");
    assert_eq!(before[0].4, Nm::from_mm(0.9).raw(), "with a 0.9mm ring");

    let text = board_as_dsl(&mut world);
    let mut back = load(&text);
    let after = vias(&mut back);

    assert_eq!(after.len(), 1, "the via itself must survive:\n{text}");
    assert_eq!(after[0].3, Nm::from_mm(0.3).raw(), "the drill is written");
    assert_eq!(
        after[0].4,
        Nm::from_mm(0.6).raw(),
        "dsl.rs says the diameter is rebuilt as twice the drill; if this \
         changed, the comment at the top of that file is now wrong:\n{text}"
    );
}
