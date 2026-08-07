//! The checker and the legend file have to be talking about the same ink.
//!
//! Silkscreen printed over a pad resists solder, so the joint under it is
//! starved or open. A board house clips the legend off solderable copper
//! before it prints, which means a file that needs clipping is a file whose
//! legend nobody has seen. The exporter clips it here instead, at the
//! clearance of the fabricator it was told about, and `silk-clearance`
//! measures what survives - so what is checked is what gets made.
//!
//! These tests run the whole way round: lay a name across a neighbour's pad,
//! export, and read the Gerber back. None of them compares two functions to
//! each other.

use cypcb_core::{Nm, Point};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::run_drc;
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::silk::{export_silkscreen_reporting, SilkConfig};
use cypcb_export::gerber::Side;
use cypcb_world::footprint::{FootprintLibrary, SilkShape};
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Build the world a source string describes, the way every command does.
fn world_from(source: &str) -> (BoardWorld, FootprintLibrary) {
    let parsed = cypcb_parser::parse(source);
    assert!(
        parsed.errors.is_empty(),
        "the board must parse: {:?}",
        parsed.errors
    );

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(
        result.errors.is_empty(),
        "the board must sync: {:?}",
        result.errors
    );
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

/// The silk violations a board reports against a given house.
fn silk_violations(world: &mut BoardWorld, rules: &DesignRules) -> Vec<String> {
    run_drc(world, rules)
        .violations
        .into_iter()
        .filter(|violation| violation.kind == cypcb_drc::violation::ViolationKind::SilkClearance)
        .map(|violation| violation.message)
        .collect()
}

/// Two 0805 parts placed so that C1's name reaches C2's copper while C1's
/// courtyard does not.
///
/// That gap is the point. An 0805 courtyard stops 0.975mm above the part's
/// origin and the name is printed above that, so at 2.2mm apart C2's lower pad
/// edge - 1.475mm above C1 - is under the text and clear of the outline. A
/// fixture that only stacks the parts closer is satisfied by the courtyard
/// check alone.
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

/// C2's pads, as (centre, half-side of the square nothing may print inside at
/// the exporter's default clearance).
///
/// 0805 pads are 1.0 x 1.45mm at x = 10 +/- 0.95. The keepout is a square of
/// `max(width, height) / 2 + clearance + half a stroke`.
fn c2_keepouts() -> Vec<((f64, f64), f64)> {
    let half = 1.45 / 2.0 + 0.13 + 0.075;
    vec![((9.05, 12.2), half), ((10.95, 12.2), half)]
}

fn inside_a_keepout(x: f64, y: f64, slack: f64) -> bool {
    c2_keepouts()
        .iter()
        .any(|((px, py), half)| (x - px).abs() < half - slack && (y - py).abs() < half - slack)
}

#[test]
fn a_name_that_would_cross_a_neighbours_pad_is_clipped_off_it() {
    let (mut world, library) = world_from(NAME_OVER_A_NEIGHBOURS_PAD);

    // The name, as it would be laid out with nothing in the way.
    let footprint = library.get("0805").expect("the 0805 footprint is built in");
    let unclipped = cypcb_world::silk_text::designator_strokes(
        "C1",
        Point::new(Nm::from_mm(10.0), Nm::from_mm(10.0)),
        Nm::from_mm(1.0),
        Nm::from_mm(0.15),
        cypcb_world::silk_text::artwork_rise(footprint, 0.0),
    );
    let crosses = unclipped.iter().any(|shape| {
        let SilkShape::Segment { start, end, .. } = shape else {
            return false;
        };
        [start, end].iter().any(|point| {
            inside_a_keepout(
                point.x.raw() as f64 / 1e6,
                point.y.raw() as f64 / 1e6,
                1e-9,
            )
        })
    });
    assert!(
        crosses,
        "the fixture has to put C1's name on C2's copper, or it tests nothing"
    );

    // And the file draws none of it there.
    let (gerber, warnings) = export_silkscreen_reporting(
        &mut world,
        &library,
        Side::Top,
        &CoordinateFormat::FORMAT_MM_2_6,
        &SilkConfig::default(),
    )
    .expect("the legend exports");
    let points = legend_points(&gerber);
    let on_copper: Vec<_> = points
        .iter()
        .filter(|(x, y)| inside_a_keepout(*x, *y, 1e-9))
        .collect();
    assert!(
        on_copper.is_empty(),
        "the legend file still puts ink inside C2's keepout: {on_copper:?}"
    );

    // C2's pads swallow both glyphs of `C1` whole, so the label is gone rather
    // than shortened - and the exporter says so instead of leaving a part
    // nobody can identify.
    assert!(
        warnings.iter().any(|warning| warning.refdes == "C1"),
        "a name the clipping removed entirely has to be reported"
    );

    // The rest of the legend survives: this clips artwork, it does not blank
    // the layer.
    assert!(
        points.len() > 20,
        "the whole legend went missing, {} points left",
        points.len()
    );
}

#[test]
fn the_checker_agrees_with_the_file_it_will_ship() {
    let (mut world, _library) = world_from(NAME_OVER_A_NEIGHBOURS_PAD);
    let silk = silk_violations(&mut world, &DesignRules::jlcpcb_2layer());

    assert!(
        silk.is_empty(),
        "the ink is clipped off the copper, so the checker has nothing to report: {silk:?}"
    );
}

/// The same two parts, with a name long enough that its strokes cross the
/// edge of C2's keepout rather than falling wholly inside it.
///
/// Where a stroke is cut decides everything below: clipping leaves ink exactly
/// on the keepout boundary, which is far enough for the house the file was
/// clipped for and not for a stricter one. A short name whose glyphs happen to
/// sit entirely inside the keepout is deleted instead, and leaves nothing to
/// measure.
const A_NAME_LONG_ENOUGH_TO_BE_CUT: &str = r#"
board silk {
    size 20mm x 20mm
    layers 2
}

component C1234567 capacitor "0805" {
    value "100nF"
    at 10mm, 10mm
}

component C2 capacitor "0805" {
    value "100nF"
    at 10mm, 12.2mm
}
"#;

#[test]
fn a_legend_clipped_for_one_house_is_reported_when_sent_to_a_stricter_one() {
    // The case that keeps this rule from being a formality. The exporter clips
    // at the clearance it was told about; check the same board against a
    // fabricator that asks for more and the ink that was fine is not.
    let (mut world, _library) = world_from(A_NAME_LONG_ENOUGH_TO_BE_CUT);

    let strict = DesignRules::pcbway_standard();
    assert!(
        strict.min_clearance > DesignRules::jlcpcb_2layer().min_clearance,
        "this test needs a house stricter than the one the exporter clips for"
    );

    let silk = silk_violations(&mut world, &strict);
    assert!(
        !silk.is_empty(),
        "a legend clipped to JLCPCB's clearance does not meet PCBWay's, and the checker has to \
         say so"
    );

    // And the house it was clipped for is still happy with it, or the two
    // numbers are not being used for what they claim.
    let (mut world, _library) = world_from(A_NAME_LONG_ENOUGH_TO_BE_CUT);
    let lenient = silk_violations(&mut world, &DesignRules::jlcpcb_2layer());
    assert!(
        lenient.is_empty(),
        "the board was clipped for this house: {lenient:?}"
    );
}

#[test]
fn a_name_the_clipping_ate_is_reported_rather_than_left_on_the_board() {
    // Clipping is only safe if the person sending the file knows what it cost
    // them. A designator eaten by the pads around it leaves a part nobody can
    // identify, and the file itself gives no sign of it.
    let (mut world, library) = world_from(A_NAME_LONG_ENOUGH_TO_BE_CUT);

    let (_gerber, warnings) = export_silkscreen_reporting(
        &mut world,
        &library,
        Side::Top,
        &CoordinateFormat::FORMAT_MM_2_6,
        &SilkConfig::default(),
    )
    .expect("the legend exports");

    let eaten = warnings
        .iter()
        .find(|warning| warning.refdes == "C1234567")
        .expect("the long name loses most of itself to C2\'s pads");

    assert!(
        eaten.strokes_drawn < eaten.strokes_wanted,
        "a name reported as unreadable has to have lost strokes"
    );
    assert!(
        eaten.strokes_drawn * 2 < eaten.strokes_wanted,
        "the warning is for names that lost more than half of themselves, this one kept {} of {}",
        eaten.strokes_drawn,
        eaten.strokes_wanted
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
    let silk = silk_violations(&mut world, &DesignRules::jlcpcb_2layer());

    assert!(
        silk.is_empty(),
        "a part alone on the board prints its name clear of its own pads: {silk:?}"
    );
}
