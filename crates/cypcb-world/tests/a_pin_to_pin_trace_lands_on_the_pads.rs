//! `trace VCC { from R1.1 to C1.1 }` has to be copper between those two pads.
//!
//! It was copper between the two part centres. `get_pin_position` returned the
//! component's own position with a comment calling it "a good approximation",
//! and that copper is what the exporter writes into the Gerber: a trace that
//! touches neither pad it names, running under the parts instead of to them.

use cypcb_core::{Nm, Point};
use cypcb_parser::parse;
use cypcb_world::components::trace::Trace;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

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

fn only_trace(world: &mut BoardWorld) -> Trace {
    let mut query = world.ecs_mut().query::<&Trace>();
    let traces: Vec<Trace> = query.iter(world.ecs()).cloned().collect();
    assert_eq!(traces.len(), 1, "the fixture draws one trace");
    traces.into_iter().next().expect("the trace")
}

/// Where a pad of a part sits on the board, computed from the library rather
/// than written down, so the test says what it means instead of repeating a
/// number the footprint could change.
fn pad_at(
    library: &FootprintLibrary,
    footprint: &str,
    pin: &str,
    at: Point,
    degrees: f64,
) -> Point {
    let pad = library
        .get(footprint)
        .expect("the footprint is registered")
        .pads
        .iter()
        .find(|pad| pad.number == pin)
        .expect("the footprint has that pin")
        .position;
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (px, py) = (pad.x.0 as f64, pad.y.0 as f64);
    Point::new(
        Nm(at.x.0 + (px * cos - py * sin).round() as i64),
        Nm(at.y.0 + (px * sin + py * cos).round() as i64),
    )
}

const TWO_PARTS: &str = r#"
board t {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    at 5mm, 10mm
}

component C1 capacitor "0402" {
    at 20mm, 10mm
    rotate 90deg
}

net VCC {
    R1.1
    C1.1
}

trace VCC {
    from R1.1
    to C1.1
    layer Top
    width 0.3mm
}
"#;

#[test]
fn the_copper_starts_and_ends_on_the_pads_the_trace_names() {
    let (mut world, library) = world_from(TWO_PARTS);
    let trace = only_trace(&mut world);

    assert_eq!(trace.segments.len(), 1, "one straight run between two pins");
    let segment = &trace.segments[0];

    let from = pad_at(&library, "0402", "1", Point::from_mm(5.0, 10.0), 0.0);
    let to = pad_at(&library, "0402", "1", Point::from_mm(20.0, 10.0), 90.0);

    assert_eq!(segment.start, from, "the copper has to start on R1.1");
    assert_eq!(segment.end, to, "and end on C1.1");
}

#[test]
fn a_turned_part_moves_its_pad_and_the_copper_follows() {
    // C1 is rotated 90 degrees, so its pad 1 leaves the x axis. A trace drawn
    // to a part centre would not notice; this is what separates the two.
    let (mut world, library) = world_from(TWO_PARTS);
    let trace = only_trace(&mut world);
    let end = trace.segments[0].end;

    let centre = Point::from_mm(20.0, 10.0);
    assert_ne!(
        end, centre,
        "the endpoint is a pad, not the part's own position"
    );
    assert_eq!(
        end,
        pad_at(&library, "0402", "1", centre, 90.0),
        "a pad of a part turned 90 degrees sits off the x axis"
    );
    assert_eq!(end.x, centre.x, "and directly above or below the centre");
}

#[test]
fn a_pin_the_footprint_does_not_have_still_draws_something() {
    // The endpoint falls back to the part's position rather than dropping the
    // copper, because a missing pin is reported where the netlist is checked
    // and a silently missing trace is the harder failure to notice.
    let source = TWO_PARTS.replace("from R1.1", "from R1.99");
    let (mut world, _library) = world_from(&source);
    let trace = only_trace(&mut world);

    assert_eq!(trace.segments[0].start, Point::from_mm(5.0, 10.0));
}
