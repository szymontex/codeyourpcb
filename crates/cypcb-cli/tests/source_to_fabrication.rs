//! The whole point of the program, in one test.
//!
//! A user writes `.cypcb`, routes it, saves it, and sends the Gerbers to a
//! board house. Every step of that chain has been tested on its own and the
//! chain itself never was, which is how two storage-format defects shipped:
//! traces written as one polyline across a net's branches, and inner layers
//! written with a number the grammar rejects. Both are invisible to any test
//! that stops before the round trip.
//!
//! This routes a board, writes it as source, reads that source back as a
//! stranger would, exports it, and counts the copper. The number that has to
//! match is the router's own segment count: what the fabricator receives is
//! what the router drew, or the program has failed at its purpose.

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::export_excellon;
use cypcb_export::gerber::{
    export_copper_layer, export_outline, export_soldermask, export_solderpaste, MaskPasteConfig,
    Side,
};
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Two parts and a net that branches to three pads, which is the shape that
/// broke the writer: the router leaves several chains of segments and they
/// must not be joined into one.
const SOURCE: &str = r#"version 1

board fab {
    size 40mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value 10kohm
    at 8mm, 10mm
}

component R2 resistor "0402" {
    value 10kohm
    at 22mm, 10mm
}

component R3 resistor "0402" {
    value 10kohm
    at 32mm, 14mm
}

net SIG {
    R1.2
    R2.1
    R3.1
}

net GND {
    R1.1
    R2.2
    R3.2
}
"#;

fn load(source: &str) -> (BoardWorld, FootprintLibrary) {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    (world, library)
}

/// Count the aperture-draw commands in a Gerber layer.
///
/// `D01` draws to a point, which is one segment of copper; `D02` only moves.
fn draws(gerber: &str) -> usize {
    gerber.lines().filter(|line| line.contains("D01")).count()
}

/// Every coordinate the layer draws to or moves to, in a comparable order.
///
/// Counting draws catches copper that goes missing. It does not catch copper
/// that moves, and the defect this file exists for did exactly that: a
/// polyline joining two of a net's branches replaces a short segment with a
/// long one and the count is unchanged.
fn strokes(gerber: &str) -> Vec<String> {
    let mut out: Vec<String> = gerber
        .lines()
        .filter(|line| line.contains("D01") || line.contains("D02"))
        .map(|line| line.trim().to_string())
        .collect();
    out.sort();
    out
}

#[test]
fn a_routed_board_survives_being_saved_and_reaches_the_gerbers() {
    let (mut world, library) = load(SOURCE);
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("jlcpcb preset"));

    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    let routed_segments = result.routes.len();
    let routed_vias = result.vias.len();
    assert!(routed_segments > 0, "the router produced nothing to save");

    apply_routes(&mut world, &result);

    // Save it the way `cypcb route` saves it: the design, then the traces.
    let saved = format!(
        "{}\n{}",
        SOURCE,
        cypcb_world::dsl::traces_as_dsl(&mut world)
    );

    // Read it back as a stranger would - a fresh world, nothing carried over.
    let (mut reloaded, reloaded_library) = load(&saved);
    reloaded.rebuild_spatial_index_from_library(&reloaded_library);

    let format = CoordinateFormat::FORMAT_MM_2_6;
    let top = export_copper_layer(&mut reloaded, &reloaded_library, Layer::TopCopper, &format)
        .expect("top copper");
    let bottom = export_copper_layer(
        &mut reloaded,
        &reloaded_library,
        Layer::BottomCopper,
        &format,
    )
    .expect("bottom copper");
    let drill = export_excellon(&mut reloaded, &reloaded_library, &format, None).expect("drill");

    // Pads draw too, so copper has to be at least the routed segments - and a
    // polyline that joined a net's branches would push it over by inventing
    // one segment per join.
    let copper = draws(&top) + draws(&bottom);
    assert!(
        copper >= routed_segments,
        "the fabricator is missing copper: {routed_segments} segments routed, {copper} drawn"
    );

    // Every via has to become a hole. There may be more holes than vias -
    // through-hole pads drill too - but never fewer.
    let holes = drill.lines().filter(|line| line.starts_with('X')).count();
    assert!(
        holes >= routed_vias,
        "the fabricator is missing holes: {routed_vias} vias routed, {holes} drilled"
    );
}

#[test]
fn saving_a_routed_board_does_not_change_how_much_copper_it_has() {
    // The strict half of the check above. Count the copper drawn straight from
    // the routed board, then from the same board after a save and reload: the
    // two have to agree exactly. This is what the branch-joining defect broke,
    // and it broke it silently - the file parsed, the export succeeded, and the
    // board had extra traces across it.
    let (mut world, library) = load(SOURCE);
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("jlcpcb preset"));

    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let format = CoordinateFormat::FORMAT_MM_2_6;
    let direct_top =
        export_copper_layer(&mut world, &library, Layer::TopCopper, &format).expect("top copper");
    let direct_bottom = export_copper_layer(&mut world, &library, Layer::BottomCopper, &format)
        .expect("bottom copper");
    let direct = draws(&direct_top) + draws(&direct_bottom);

    let saved = format!(
        "{}\n{}",
        SOURCE,
        cypcb_world::dsl::traces_as_dsl(&mut world)
    );
    let (mut reloaded, reloaded_library) = load(&saved);
    reloaded.rebuild_spatial_index_from_library(&reloaded_library);

    let reloaded_top =
        export_copper_layer(&mut reloaded, &reloaded_library, Layer::TopCopper, &format)
            .expect("top copper");
    let reloaded_bottom = export_copper_layer(
        &mut reloaded,
        &reloaded_library,
        Layer::BottomCopper,
        &format,
    )
    .expect("bottom copper");
    let after_round_trip = draws(&reloaded_top) + draws(&reloaded_bottom);

    assert_eq!(
        after_round_trip, direct,
        "saving and reloading changed how much copper the board has"
    );
    assert_eq!(
        strokes(&reloaded_top),
        strokes(&direct_top),
        "saving and reloading moved copper on the top layer"
    );
    assert_eq!(
        strokes(&reloaded_bottom),
        strokes(&direct_bottom),
        "saving and reloading moved copper on the bottom layer"
    );
}

/// The same board with a corner taken out of it.
///
/// A fabricator cuts to `Edge_Cuts`, so a board that states an outline and
/// gets a rectangle back is not the board that was ordered.
const CUTOUT: &str = r#"version 1

board fab {
    size 40mm x 30mm
    layers 2
}

outline {
    point 0mm, 0mm
    point 40mm, 0mm
    point 40mm, 15mm
    point 20mm, 15mm
    point 20mm, 30mm
    point 0mm, 30mm
}

component R1 resistor "0402" {
    value 10kohm
    at 8mm, 8mm
}
"#;

#[test]
fn the_fabricator_cuts_the_board_the_source_declared() {
    let (world, _library) = load(CUTOUT);
    let format = CoordinateFormat::FORMAT_MM_2_6;

    let edge = export_outline(&world, &format).expect("board outline");

    // Six corners and back to the first: one pen-up, six draws.
    let moves = edge.lines().filter(|line| line.contains("D02")).count();
    let cuts = edge.lines().filter(|line| line.contains("D01")).count();
    assert_eq!(moves, 1, "the outline is one closed path:\n{edge}");
    assert_eq!(
        cuts, 6,
        "six corners means six cuts back to the start:\n{edge}"
    );

    // The distinct X coordinates are what separates this board from the
    // rectangle it would be mistaken for: a rectangle has two, this has three.
    let xs: std::collections::BTreeSet<&str> = edge
        .lines()
        .filter(|line| line.contains("D01") || line.contains("D02"))
        .filter_map(|line| line.split('Y').next())
        .map(|x| x.trim_start_matches('X'))
        .collect();
    assert_eq!(
        xs.len(),
        3,
        "a board with a cutout has three distinct X coordinates, a rectangle two:\n{edge}"
    );
    // Written the way `%FSLAX26Y26*%` declares: six implied decimals, no
    // point. `0`, not `0.000000` - leading zeros are suppressed, which is what
    // the `L` in the format declaration means.
    for x in ["0", "20000000", "40000000"] {
        // 0mm, 20mm, 40mm in 2.6
        assert!(
            xs.contains(x),
            "the outline is missing the corner at X{x}: {xs:?}"
        );
    }
}

/// Every coordinate a layer flashes a pad at.
///
/// `D03` is a flash: the aperture is stamped once, which is what a pad is.
fn flashes(gerber: &str) -> std::collections::BTreeSet<String> {
    gerber
        .lines()
        .filter(|line| line.contains("D03"))
        .map(|line| line.trim().to_string())
        .collect()
}

/// The diameters of every circular aperture the layer defines, in millimetres.
fn aperture_sizes(gerber: &str) -> Vec<f64> {
    gerber
        .lines()
        .filter_map(|line| line.strip_prefix("%ADD"))
        .filter_map(|rest| rest.split(',').nth(1))
        .filter_map(|size| size.trim_end_matches("*%").split('X').next())
        .filter_map(|value| value.parse::<f64>().ok())
        .collect()
}

#[test]
fn every_pad_gets_an_opening_in_the_soldermask() {
    // Solder mask is what keeps solder off everything that is not a pad. A
    // mask that does not match the copper produces a board that bridges when
    // it is assembled, and nothing had ever compared the two files.
    //
    // CUTOUT carries one 0402 and no traces or vias, so every flash in the
    // copper layer is a pad and the two sets have to be identical.
    let (mut world, library) = load(CUTOUT);
    let format = CoordinateFormat::FORMAT_MM_2_6;

    let copper =
        export_copper_layer(&mut world, &library, Layer::TopCopper, &format).expect("top copper");
    let mask = export_soldermask(
        &mut world,
        &library,
        Side::Top,
        &format,
        &MaskPasteConfig::default(),
    )
    .expect("top soldermask");

    let pads = flashes(&copper);
    assert_eq!(pads.len(), 2, "an 0402 has two pads:\n{copper}");
    assert_eq!(
        flashes(&mask),
        pads,
        "the mask openings do not sit where the pads do"
    );

    // And each opening has to be larger than the pad it uncovers, or the
    // slightest misregistration covers copper the assembler needs.
    let widest_pad = aperture_sizes(&copper).into_iter().fold(0.0f64, f64::max);
    let widest_opening = aperture_sizes(&mask).into_iter().fold(0.0f64, f64::max);
    assert!(
        widest_opening > widest_pad,
        "the mask opening ({widest_opening}mm) has to be wider than the pad ({widest_pad}mm)"
    );
}

#[test]
fn the_paste_stencil_sits_on_the_pads_and_can_be_made_smaller_than_them() {
    // A stencil aperture cut to pad size deposits as much solder as the pad
    // can hold, which is how a 0402 tombstones: one end wets before the other
    // and the part stands up. The apertures have to sit on the pads, and the
    // reduction that keeps them under pad size has to reach the output.
    let (mut world, library) = load(CUTOUT);
    let format = CoordinateFormat::FORMAT_MM_2_6;

    let copper =
        export_copper_layer(&mut world, &library, Layer::TopCopper, &format).expect("top copper");
    let one_to_one = export_solderpaste(
        &mut world,
        &library,
        Side::Top,
        &format,
        &MaskPasteConfig::default(),
    )
    .expect("top paste");

    assert_eq!(
        flashes(&one_to_one),
        flashes(&copper),
        "the stencil openings do not sit where the pads do"
    );

    let reduced = export_solderpaste(
        &mut world,
        &library,
        Side::Top,
        &format,
        &MaskPasteConfig::default().with_paste_reduction(0.1),
    )
    .expect("top paste, reduced");

    let full = aperture_sizes(&one_to_one)
        .into_iter()
        .fold(0.0f64, f64::max);
    let shrunk = aperture_sizes(&reduced).into_iter().fold(0.0f64, f64::max);
    assert!(
        shrunk < full,
        "a 10% reduction has to make the aperture smaller: {shrunk}mm against {full}mm"
    );

    // The shipped default is 1:1, which is a fab's choice to make and not a
    // defect - but it is a choice, so it is asserted rather than assumed.
    let pad = aperture_sizes(&copper).into_iter().fold(0.0f64, f64::max);
    assert!(
        (full - pad).abs() < 1e-9,
        "the default stencil is cut to pad size: {full}mm against {pad}mm"
    );
}

#[test]
fn the_pick_and_place_says_where_and_which_way_up() {
    // A CPL with the wrong side or the wrong rotation puts every part down
    // turned or on the wrong face, and it is three columns of numbers nobody
    // reads until the boards come back. This checks all three against the
    // design.
    use cypcb_export::cpl::export_cpl;
    use cypcb_world::components::{FootprintRef, Position, RefDes, Rotation, Side, Value};

    let (mut world, library) = load(CUTOUT);

    // A second part, deliberately on the far face and turned a quarter turn.
    // `Side` is what the design states; the footprint is the same one R1 uses,
    // so nothing about the pads gives the answer away.
    let r2 = world.spawn_component(
        RefDes::new("R2"),
        Value::new("10k"),
        Position::from_mm(30.0, 20.0),
        Rotation(90_000),
        FootprintRef::new("0402"),
        cypcb_world::components::NetConnections::new(),
    );
    world.ecs_mut().entity_mut(r2).insert(Side::Bottom);

    let cpl = export_cpl(&mut world, &library, None).expect("pick and place");

    let row = cpl
        .lines()
        .find(|line| line.starts_with("R2,"))
        .unwrap_or_else(|| panic!("R2 is missing from the CPL:\n{cpl}"));
    let fields: Vec<&str> = row.split(',').collect();

    assert!(
        row.contains("30.0") && row.contains("20.0"),
        "R2 sits at 30mm, 20mm and the CPL says {row}"
    );
    assert!(
        fields.iter().any(|f| f.trim() == "90"),
        "R2 is turned a quarter turn and the CPL says {row}"
    );
    assert!(
        fields.iter().any(|f| f.trim() == "Bottom"),
        "R2 is assembled on the bottom face and the CPL says {row}"
    );

    // And the part that says nothing is still on top.
    let r1 = cpl
        .lines()
        .find(|line| line.starts_with("R1,"))
        .unwrap_or_else(|| panic!("R1 is missing from the CPL:\n{cpl}"));
    assert!(r1.contains("Top"), "R1 is a top-side part: {r1}");
}

/// A board whose parts share values on purpose: two 10k resistors that must
/// group into one line, and one 100nF that must not join them.
const PURCHASING: &str = r#"version 1

board fab {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value 10kohm
    at 6mm, 10mm
}

component R2 resistor "0402" {
    value 10kohm
    at 14mm, 10mm
}

component C1 capacitor "0402" {
    value 100nF
    at 22mm, 10mm
}
"#;

#[test]
fn the_bom_accounts_for_every_part_exactly_once() {
    // A purchaser orders from this file. A part missing from it does not get
    // bought and the board cannot be built; a part counted twice is money
    // spent on a component nobody needs. Grouping is the point of the file -
    // two identical resistors are one line of quantity two - so the check is
    // on the total accounted for, not on the number of lines.
    use cypcb_export::bom::export_bom_csv;

    let (mut world, _library) = load(PURCHASING);
    let bom = export_bom_csv(&mut world).expect("bill of materials");

    for refdes in ["R1", "R2", "C1"] {
        let mentions = bom.matches(refdes).count();
        assert_eq!(
            mentions, 1,
            "{refdes} has to appear exactly once in the BOM:\n{bom}"
        );
    }

    // The two resistors share a value and a footprint, so they are one line.
    let lines: Vec<&str> = bom
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(
        lines.len(),
        2,
        "two 10k resistors group into one line and the capacitor makes two:\n{bom}"
    );

    let resistors = lines
        .iter()
        .find(|line| line.contains("R1"))
        .expect("the resistor line");
    assert!(
        resistors.contains("R2"),
        "both resistors belong to the same line: {resistors}"
    );
    assert!(
        resistors.contains(",2,") || resistors.contains(",2\r") || resistors.ends_with(",2"),
        "the resistor line has to be quantity two: {resistors}"
    );

    // And the values the source states have to survive into the file a
    // purchaser reads.
    assert!(bom.contains("10k"), "the resistor value is missing:\n{bom}");
    assert!(
        bom.contains("100nF"),
        "the capacitor value is missing:\n{bom}"
    );
}

#[test]
fn the_legend_puts_each_part_on_the_side_it_is_assembled_on() {
    // Silkscreen is printed per face. A part drawn on the wrong one leaves the
    // assembler with a legend that does not match what is in front of them,
    // and the board it describes cannot be checked by eye.
    //
    // The layer carries courtyard outlines and position markers; designator
    // text is not implemented, which is recorded in the tracker as a gap
    // rather than tested for here.
    use cypcb_export::gerber::{export_silkscreen, SilkConfig};
    use cypcb_world::components::{
        FootprintRef, Position, RefDes, Rotation, Side as PartSide, Value,
    };

    let (mut world, library) = load(CUTOUT);

    let underside = world.spawn_component(
        RefDes::new("R9"),
        Value::new("10k"),
        Position::from_mm(30.0, 20.0),
        Rotation(0),
        FootprintRef::new("0402"),
        cypcb_world::components::NetConnections::new(),
    );
    world
        .ecs_mut()
        .entity_mut(underside)
        .insert(PartSide::Bottom);

    let format = CoordinateFormat::FORMAT_MM_2_6;
    let config = SilkConfig::default();
    let top = export_silkscreen(&mut world, &library, Side::Top, &format, &config)
        .expect("top silkscreen");
    let bottom = export_silkscreen(&mut world, &library, Side::Bottom, &format, &config)
        .expect("bottom silkscreen");

    // R1 sits at 8mm, 8mm on top; R9 at 30mm, 20mm underneath. Each legend
    // draws around its own parts and not around the other side's.
    //
    // Asked as a distance rather than as `top.contains("X8.")`, which is how
    // these four read before the exporter stopped writing a decimal point the
    // format declaration said would not be there. A prefix match on a
    // coordinate is a bad question in any case: `X8.` also matches 8.9mm, and
    // misses 8mm written exactly.
    fn draws_near(gerber: &str, x_mm: f64) -> bool {
        gerber
            .lines()
            .filter(|line| line.contains("D01") || line.contains("D02"))
            .filter_map(|line| {
                let rest = line.strip_prefix('X')?;
                let end = rest.find(|c: char| !c.is_ascii_digit() && c != '-')?;
                // 2.6 format: six implied decimals.
                Some(rest[..end].parse::<f64>().ok()? / 1_000_000.0)
            })
            .any(|x| (x - x_mm).abs() < 1.5)
    }

    assert!(
        draws_near(&top, 8.0),
        "the top legend has to draw around R1 at 8mm:\n{top}"
    );
    assert!(
        !draws_near(&top, 30.0),
        "the top legend must not draw around a part assembled underneath:\n{top}"
    );
    assert!(
        draws_near(&bottom, 30.0),
        "the bottom legend has to draw around R9 at 30mm:\n{bottom}"
    );
    assert!(
        !draws_near(&bottom, 8.0),
        "the bottom legend must not draw around a top-side part:\n{bottom}"
    );
}

#[test]
fn saving_a_routed_board_does_not_change_what_the_checker_says_about_it() {
    round_trip_says_the_same_thing(SOURCE);
}

/// The project's own example, which is the board the numbers in the tracker
/// came from and a busier one than the fixture above.
#[test]
fn the_blink_example_reads_back_as_the_board_it_was() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/blink.cypcb");
    let source = std::fs::read_to_string(&path).expect("the example is in the repo");
    round_trip_says_the_same_thing(&source);
}

fn round_trip_says_the_same_thing(source: &str) {
    // `cypcb route` reported 5 violations in memory and `cypcb check` reported
    // 6 for the file it had just written. The Gerber strokes are proven
    // identical across that round trip, so whatever the file gained is not
    // copper - and a board that reads back as a different board is the defect
    // this suite exists for, whatever the difference turns out to be.
    use cypcb_drc::{run_drc, DesignRules};
    use std::collections::BTreeSet;

    let (mut world, library) = load(source);
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("jlcpcb preset"));
    let drc_rules = DesignRules::jlcpcb_2layer();

    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let describe = |world: &mut cypcb_world::BoardWorld| -> BTreeSet<String> {
        run_drc(world, &drc_rules)
            .violations
            .iter()
            .map(|v| {
                // The pair reads in whichever order the entities were indexed,
                // and indices differ between a routed board and the same board
                // parsed back, so `A vs B` and `B vs A` are one fact. Sorted so
                // the comparison is about the board rather than about ids.
                let message = match v.message.split_once('\u{2194}') {
                    Some((left, rest)) => match rest.split_once(':') {
                        Some((right, tail)) => {
                            let mut pair = [left.trim(), right.trim()];
                            pair.sort_unstable();
                            format!("{} <-> {}:{}", pair[0], pair[1], tail)
                        }
                        None => v.message.clone(),
                    },
                    None => v.message.clone(),
                };
                format!(
                    "{} at {:.3},{:.3}: {}",
                    v.kind,
                    v.location.x.to_mm(),
                    v.location.y.to_mm(),
                    message
                )
            })
            .collect()
    };

    let before = describe(&mut world);

    let saved = format!(
        "{}\n{}",
        source,
        cypcb_world::dsl::traces_as_dsl(&mut world)
    );
    let (mut reloaded, reloaded_library) = load(&saved);
    reloaded.rebuild_spatial_index_from_library(&reloaded_library);
    let after = describe(&mut reloaded);

    // Nothing may be lost. A board that reads back reporting less than it did
    // is a board whose file hides a fault.
    //
    // More is allowed, and on examples/blink.cypcb there is more: in memory
    // every segment a net has on a layer lives in one `Trace` entity, and the
    // clearance rule reports once per entity pair with the closest distance it
    // found - so a component too close to a net in two places is reported
    // once. Written out, that net becomes several `trace` blocks and both
    // places are named. The round trip does not add a fault; it stops the
    // checker from merging two into one. That is recorded in the tracker as
    // the next thing to fix, in the checker rather than in the file.
    // Equal, both ways. The checker reports per offending segment now rather
    // than once per pair of entities, so the count is a property of the board
    // instead of a property of how its copper is grouped - which is what made
    // a saved board report more than the board it came from.
    let lost: Vec<&String> = before.difference(&after).collect();
    let gained: Vec<&String> = after.difference(&before).collect();
    assert!(
        lost.is_empty() && gained.is_empty(),
        "the board reads back as a different board.\n  lost: {lost:#?}\n  gained: {gained:#?}"
    );
}
