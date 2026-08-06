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
use cypcb_export::gerber::export_copper_layer;
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
