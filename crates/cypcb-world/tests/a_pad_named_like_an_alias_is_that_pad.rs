//! A pin the design names is the pad of that name, before it is anything else.
//!
//! `cargo test -p cypcb-world --test a_pad_named_like_an_alias_is_that_pad`
//!
//! The language lets a diode be wired as `D1.A` and `D1.K` and a transistor as
//! `Q1.B`, `Q1.C`, `Q1.E` on footprints that number their pads, and reads those
//! letters as numbers. Until 2026-09-25 it read them as numbers always, so a
//! footprint whose pads really are named `A`, `K` or `C` could not be wired by
//! those names: a diode drawn with pads `A` and `K` was refused as having no
//! pin `1`, and on a part with pads `1`, `2` and `C` the pin `U1.C` went to pad
//! 2 without a word.

use cypcb_core::Point;
use cypcb_parser::parse;
use cypcb_world::components::trace::Trace;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld, NetConnections};

/// Build the world a source file describes, and return it with its library.
fn world_from(source: &str) -> (BoardWorld, FootprintLibrary) {
    let parsed = parse(source);
    assert!(
        parsed.errors.is_empty(),
        "the fixture has to parse: {:?}",
        parsed.errors
    );

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(
        result.errors.is_empty(),
        "the fixture has to sync: {:?}",
        result.errors
    );
    (world, library)
}

/// The pad each pin of `refdes` was put on, and the net it carries.
fn pins_of(world: &mut BoardWorld, refdes: &str) -> Vec<(String, String)> {
    let entity = world.find_by_refdes(refdes).expect("the part exists");
    let mut pins: Vec<(String, String)> = world
        .get::<NetConnections>(entity)
        .expect("the part has connections")
        .iter()
        .map(|connection| {
            let net = world
                .net_name(connection.net)
                .expect("the net is interned")
                .to_string();
            (connection.pin.clone(), net)
        })
        .collect();
    pins.sort();
    pins
}

fn pinned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(pin, net)| (pin.to_string(), net.to_string()))
        .collect()
}

/// Pads `1`, `2` and `C`: the letter a transistor alias reads as `2` is also a
/// pad this part really has.
const PADS_1_2_AND_C: &str = r#"
board t {
    size 30mm x 20mm
    layers 2
}

footprint THREE_AND_C {
    courtyard 6mm x 2mm
    pad 1 rect at -2mm, 0mm size 1mm x 1mm
    pad 2 rect at 0mm, 0mm size 1mm x 1mm
    pad C rect at 2mm, 0mm size 1mm x 1mm
}

component U1 ic "THREE_AND_C" {
    at 8mm, 10mm
}

component R1 resistor "0402" {
    at 20mm, 10mm
}

net ONE {
    U1.1
    R1.1
}

net CC {
    U1.C
    R1.2
}

trace CC {
    from U1.C
    to R1.2
    layer Top
    width 0.3mm
}
"#;

#[test]
fn a_pin_named_c_lands_on_the_pad_named_c() {
    let (mut world, _library) = world_from(PADS_1_2_AND_C);

    assert_eq!(
        pins_of(&mut world, "U1"),
        pinned(&[("1", "ONE"), ("C", "CC")]),
        "U1.C is pad C, and pad 2 is on no net"
    );
}

#[test]
fn a_trace_to_a_pad_named_c_ends_on_that_pad() {
    let (mut world, _library) = world_from(PADS_1_2_AND_C);
    let mut query = world.ecs_mut().query::<&Trace>();
    let traces: Vec<Trace> = query.iter(world.ecs()).cloned().collect();
    assert_eq!(traces.len(), 1, "the fixture draws one trace");

    // U1 sits at 8mm, 10mm unturned; pad C is 2mm right of it and pad 2 is
    // under its centre.
    assert_eq!(
        traces[0].segments[0].start,
        Point::from_mm(10.0, 10.0),
        "the copper starts on pad C, not on pad 2"
    );
}

#[test]
fn a_diode_drawn_with_pads_a_and_k_is_wired_by_those_names() {
    let source = r#"
board t {
    size 30mm x 20mm
    layers 2
}

footprint DIODE_AK {
    courtyard 4mm x 2mm
    pad A rect at -1mm, 0mm size 1mm x 1mm
    pad K rect at 1mm, 0mm size 1mm x 1mm
}

component D1 ic "DIODE_AK" {
    at 8mm, 10mm
}

net IN {
    D1.A
}

net OUT {
    D1.K
}
"#;
    let (mut world, _library) = world_from(source);

    assert_eq!(
        pins_of(&mut world, "D1"),
        pinned(&[("A", "IN"), ("K", "OUT")])
    );
}

/// The reason the aliases exist: a diode or LED footprint numbers its pads,
/// anode 1 and cathode 2, and a design names them by what they are.
#[test]
fn a_diode_numbered_1_and_2_still_takes_a_and_k() {
    let source = r#"
board t {
    size 30mm x 20mm
    layers 2
}

component LED1 led "0603" {
    at 10mm, 10mm
}

net IN {
    LED1.A
}

net OUT {
    LED1.K
}
"#;
    let (mut world, _library) = world_from(source);

    assert_eq!(
        pins_of(&mut world, "LED1"),
        pinned(&[("1", "IN"), ("2", "OUT")])
    );
}
