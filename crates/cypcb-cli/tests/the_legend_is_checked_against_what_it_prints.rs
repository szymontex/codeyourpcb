//! The checker and the legend file have to be talking about the same ink.
//!
//! Silkscreen printed over a pad resists solder, so the joint under it is
//! starved or open. `silk-clearance` exists to catch that, and for as long as
//! designators were laid out inside `cypcb-export` the rule could not see
//! them: it measured courtyard outlines while the exporter printed part names
//! the rule had never heard of. Both read `cypcb_world::silk_text` now.
//!
//! This is the test that says so, end to end and in the direction that
//! matters. It does not compare two functions - it puts a name over a pad,
//! asks the checker whether the board is fit to make, and then reads the
//! Gerber to confirm the ink really is where the checker said it was.

use cypcb_drc::presets::DesignRules;
use cypcb_drc::run_drc;
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::silk::{export_silkscreen, SilkConfig};
use cypcb_export::gerber::Side;
use cypcb_core::{Nm, Point};
use cypcb_world::footprint::{FootprintLibrary, SilkShape};
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Build the world a source string describes, the way every command does.
fn world_from(source: &str) -> (BoardWorld, FootprintLibrary) {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "the board must parse: {:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "the board must sync: {:?}", result.errors);
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

/// Every point the legend file draws to or moves to, in millimetres.
fn legend_points(gerber: &str) -> Vec<(f64, f64)> {
    gerber
        .lines()
        .filter_map(|line| {
            let line = line.strip_suffix('*')?;
            let (x, rest) = line.strip_prefix('X')?.split_once('Y')?;
            let y = rest.strip_suffix("D01").or_else(|| rest.strip_suffix("D02"))?;
            // The exporter writes millimetres with an explicit decimal point.
            Some((x.parse::<f64>().ok()?, y.parse::<f64>().ok()?))
        })
        .collect()
}

/// Two 0805 parts placed so that C1's *name* reaches C2's copper while C1's
/// *courtyard* does not.
///
/// That gap is the whole point. An 0805 courtyard stops 0.875mm above the
/// part's origin and the name is printed from 1.175mm to 2.175mm above it, so
/// at 2.2mm apart C2's lower pad edge - 1.475mm above C1 - is under the text
/// and clear of the outline. A test that only stacks the parts closer is
/// passed by the courtyard check alone, which is what the first version of
/// this file did.
const NAME_OVER_A_NEIGHBOURS_PAD: &str = r#"
board silk {
    size 20mm x 20mm
    layers 2
}

component C1 capacitor "0805" {
    value "100nF"
    at 10mm, 10mm
}

component C2 capacitor "0805" {
    value "100nF"
    at 10mm, 12.2mm
}
"#;

#[test]
fn a_name_printed_over_a_neighbours_pad_is_reported() {
    let (mut world, _library) = world_from(NAME_OVER_A_NEIGHBOURS_PAD);
    let violations = run_drc(&mut world, &DesignRules::jlcpcb_2layer()).violations;

    let silk: Vec<_> = violations
        .iter()
        .filter(|violation| violation.kind == cypcb_drc::violation::ViolationKind::SilkClearance)
        .collect();

    assert!(
        silk.iter().any(|violation| violation.message.contains("C1 silkscreen over C2")),
        "C1's name lands on C2's copper and the checker has to say so. Reported: {:?}",
        violations.iter().map(|v| &v.message).collect::<Vec<_>>()
    );
}

#[test]
fn the_ink_the_checker_measured_is_the_ink_the_file_draws() {
    // The half of the claim a checker cannot make about itself: every stroke
    // the model lays out has to be in the Gerber the board house receives. A
    // second layout inside the exporter would pass a looser test and still
    // print the name somewhere the rule never looked.
    let (mut world, library) = world_from(NAME_OVER_A_NEIGHBOURS_PAD);
    let gerber = export_silkscreen(
        &mut world,
        &library,
        Side::Top,
        &CoordinateFormat::FORMAT_MM_2_6,
        &SilkConfig::default(),
    )
    .expect("the legend exports");

    let points = legend_points(&gerber);
    assert!(!points.is_empty(), "the legend file has to draw something");

    let footprint = library.get("0805").expect("the 0805 footprint is built in");
    let expected = cypcb_world::silk_text::designator_strokes(
        "C1",
        Point::new(Nm::from_mm(10.0), Nm::from_mm(10.0)),
        Nm::from_mm(1.0),
        Nm::from_mm(0.15),
        cypcb_world::silk_text::artwork_rise(footprint, 0.0),
    );
    assert!(!expected.is_empty(), "C1 has a name this font can spell");

    let mut missing = Vec::new();
    for shape in &expected {
        let SilkShape::Segment { start, end, .. } = shape else {
            continue;
        };
        for point in [start, end] {
            let mm = (point.x.raw() as f64 / 1e6, point.y.raw() as f64 / 1e6);
            let found = points
                .iter()
                .any(|(x, y)| (x - mm.0).abs() < 1e-6 && (y - mm.1).abs() < 1e-6);
            if !found {
                missing.push(mm);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "the model laid out strokes the legend file does not draw: {missing:?}"
    );

    // And the ink really is on C2's copper, which is what the checker said.
    // C2's lower pads span y 11.475..12.925mm, x 8.55..9.55 and 10.45..11.45.
    let on_c2_copper = points.iter().any(|(x, y)| {
        (11.475..=12.925).contains(y) && ((8.55..=9.55).contains(x) || (10.45..=11.45).contains(x))
    });
    assert!(
        on_c2_copper,
        "the checker reported ink on C2's pad; the legend file has to contain it"
    );
}

/// A part on its own, far from anything, must not be reported - otherwise the
/// rule fires on every board and nobody reads it.
#[test]
fn a_name_that_clears_its_own_part_is_not_reported() {
    let source = r#"
board silk {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 10mm, 10mm
}
"#;

    let (mut world, _library) = world_from(source);
    let violations = run_drc(&mut world, &DesignRules::jlcpcb_2layer()).violations;

    let silk: Vec<_> = violations
        .iter()
        .filter(|violation| violation.kind == cypcb_drc::violation::ViolationKind::SilkClearance)
        .map(|violation| violation.message.clone())
        .collect();

    assert!(
        silk.is_empty(),
        "a part alone on the board prints its name clear of its own pads: {silk:?}"
    );
}
