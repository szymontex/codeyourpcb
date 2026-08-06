//! The DSL is the storage format, so what it writes has to be what it reads.
//!
//! The project's claim is that traces persist as readable code and survive a
//! round trip exactly. This checks that end to end: route a board, write the
//! result as `.cypcb`, parse it back, and compare the geometry that comes out
//! against the geometry that went in.

use std::collections::BTreeSet;

use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::dsl::traces_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Every trace segment on the board, as comparable integers.
///
/// Nanometres, the net's name rather than its id - ids depend on the order
/// nets were interned, which a round trip has no reason to preserve.
fn segments(world: &mut BoardWorld) -> BTreeSet<(String, String, i64, i64, i64, i64, i64)> {
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };

    let mut out = BTreeSet::new();
    for trace in traces {
        let net = world
            .net_name(trace.net_id)
            .unwrap_or("unnamed")
            .to_string();
        for segment in &trace.segments {
            out.insert((
                net.clone(),
                format!("{:?}", trace.layer),
                segment.start.x.raw(),
                segment.start.y.raw(),
                segment.end.x.raw(),
                segment.end.y.raw(),
                trace.width.raw(),
            ));
        }
    }
    out
}

/// Every via, likewise.
fn vias(world: &mut BoardWorld) -> BTreeSet<(String, i64, i64, i64)> {
    let placed: Vec<Via> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        query.iter(ecs).copied().collect()
    };

    placed
        .into_iter()
        .map(|via| {
            let net = world.net_name(via.net_id).unwrap_or("unnamed").to_string();
            (
                net,
                via.position.x.raw(),
                via.position.y.raw(),
                via.drill.raw(),
            )
        })
        .collect()
}

fn load(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    world
}

/// A board with hand-written traces on both layers and a via, which is the
/// shape the router emits.
const ROUTED: &str = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value 10kohm
    at 10mm, 10mm
}

component R2 resistor "0402" {
    value 10kohm
    at 30mm, 10mm
}

net SIG {
    R1.2
    R2.1
}

trace SIG {
    layer Top
    width 0.127000mm
    path 10.500000mm,10.000000mm -> 20.000000mm,10.000000mm
}

trace SIG {
    layer Bottom
    width 0.250000mm
    path 20.000000mm,10.000000mm -> 29.500000mm,10.000000mm
}

trace SIG {
    via 20.000000mm,10.000000mm drill 0.300000mm
}
"#;

#[test]
fn writing_a_routed_board_and_reading_it_back_changes_nothing() {
    let mut first = load(ROUTED);
    let before_segments = segments(&mut first);
    let before_vias = vias(&mut first);

    assert!(
        !before_segments.is_empty(),
        "no segments parsed from the fixture"
    );
    assert!(
        !before_vias.is_empty(),
        "no vias parsed from the fixture: {} segments did parse",
        before_segments.len()
    );

    // Write the board back out, and read what was written.
    let written = traces_as_dsl(&mut first);
    let header = ROUTED
        .split("trace SIG {")
        .next()
        .expect("everything before the first trace");
    let round_tripped = format!("{header}{written}");

    let mut second = load(&round_tripped);

    assert_eq!(
        segments(&mut second),
        before_segments,
        "a segment changed between writing and reading"
    );
    assert_eq!(
        vias(&mut second),
        before_vias,
        "a via changed between writing and reading"
    );
}

#[test]
fn a_second_pass_writes_the_same_bytes() {
    // Not just equal geometry - equal text. A format whose output depends on
    // how many times it has been through the tool is not a storage format, and
    // a diff of two routed files would be unreadable.
    let mut first = load(ROUTED);
    let once = traces_as_dsl(&mut first);

    let header = ROUTED.split("trace SIG {").next().unwrap();
    let mut second = load(&format!("{header}{once}"));
    let twice = traces_as_dsl(&mut second);

    assert_eq!(once, twice, "the writer is not idempotent");
}

#[test]
fn a_branching_net_does_not_gain_copper_between_its_branches() {
    // The router hands `apply_routes` every segment a net has, and it stores
    // them as one `Trace` per (net, layer) - so a net with three pads holds two
    // chains that do not join. Writing that as a single polyline draws a line
    // from the end of one chain to the start of the next: copper nobody
    // routed, and a short wherever it lands. Measured on examples/blink.cypcb
    // before this was fixed, the routed board had 2 DRC violations and the
    // file written from it had 13.
    //
    // Built by hand rather than parsed, because parsing two `trace` blocks
    // makes two entities - only the router's own path produces the shape that
    // breaks.
    use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
    use cypcb_world::components::Layer;

    let mut world = BoardWorld::new();
    world.set_board(
        "t".to_string(),
        (cypcb_core::Nm::from_mm(40.0), cypcb_core::Nm::from_mm(20.0)),
        2,
    );
    let net = world.intern_net("SIG");
    world.spawn_entity((
        Trace {
            segments: vec![
                TraceSegment::new(
                    cypcb_core::Point::from_mm(10.5, 10.0),
                    cypcb_core::Point::from_mm(19.5, 10.0),
                ),
                // A second chain, five millimetres away and not touching.
                TraceSegment::new(
                    cypcb_core::Point::from_mm(25.0, 15.0),
                    cypcb_core::Point::from_mm(29.5, 15.0),
                ),
            ],
            width: cypcb_core::Nm::from_mm(0.127),
            layer: Layer::TopCopper,
            net_id: net,
            locked: false,
            source: TraceSource::Autorouted,
        },
        net,
    ));

    let before = segments(&mut world);
    assert_eq!(before.len(), 2, "two disjoint segments went in");

    let header = ROUTED.split("trace SIG {").next().unwrap();
    let written = traces_as_dsl(&mut world);
    let mut second = load(&format!("{header}{written}"));

    assert_eq!(
        segments(&mut second),
        before,
        "writing a branching net and reading it back changed its copper"
    );
}
