//! Repair re-routes the board, and only a board that ranks ahead is kept.
//!
//! `cargo test -p cypcb-autoroute --test repair_routes_the_board_again`
//!
//! From 2026-08-07 to 2026-09-26 every repair attempt routed nothing. The
//! attempt ran on a world still carrying the copper `measure` had laid, the
//! router leaves alone a net that copper already joins, and so each attempt
//! came back complete with no routes at all - which has no contacts, and was
//! kept as a repair. Nothing noticed, because nothing turned repair on.

use std::path::Path;

use cypcb_autoroute::route_board;
use cypcb_autoroute::scoring::{score_board, ScoreWeights};
use cypcb_autoroute::variant::{default_variant_configs, generate_variants, RankKey};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::{preset_for_world, ruleset_for_world};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;
use cypcb_router::types::RoutingResult;
use cypcb_rules::presets::RulesPreset;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

fn board(filename: &str) -> (BoardWorld, FootprintLibrary) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/benchmark")
        .join(filename);
    let parsed = parse_kicad_pcb(&path).unwrap_or_else(|e| panic!("{filename}: {e:?}"));
    let library = parsed.library.clone();
    let mut world = parsed.world;
    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

/// Route with one variant's settings and `repair_passes` of repair, and rank it.
fn routed(filename: &str, variant: &str, repair_passes: u32) -> (RoutingResult, RankKey) {
    let (mut world, library) = board(filename);
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let rules = ruleset_for_world(preset, &world);
    let design_rules = DesignRules::from_constraints(&preset.constraints());
    let config = default_variant_configs()
        .into_iter()
        .find(|config| config.name == variant)
        .unwrap_or_else(|| panic!("no variant named {variant}"))
        .autoroute_config(repair_passes);

    let result = route_board(&mut world, &library, &rules, &config);
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);
    let score = score_board(&mut world, &design_rules, &ScoreWeights::default());
    (result, RankKey::of(0, &score))
}

/// `shift_driver`'s winner, repaired, ranks ahead of itself: 11 violations
/// to 8 when this was written. An attempt that routes nothing never can.
#[test]
fn a_repaired_board_ranks_ahead_of_the_board_it_repaired() {
    let variant = "PathFinder Priced Via Rings Near Holes";
    let (plain, before) = routed("shift_driver.kicad_pcb", variant, 0);
    let (repaired, after) = routed("shift_driver.kicad_pcb", variant, 2);

    assert!(
        !repaired.routes.is_empty(),
        "the repaired board carries no copper"
    );
    assert_eq!(
        after.cmp_rank(&before),
        std::cmp::Ordering::Less,
        "repair kept nothing better: {before:?} before, {after:?} after, {} routes before, {} after",
        plain.routes.len(),
        repaired.routes.len()
    );
}

/// Repair keeps an attempt only when the ranking puts it ahead. Kept by its own
/// count of contacts, `plane_board`'s `Tight Pads` came back with 3 shorts
/// where it had gone in with 2: fewer contacts, one of them now a short.
#[test]
fn repair_never_hands_back_a_board_that_ranks_behind() {
    let variant = "PathFinder Tight Pads";
    let (_, before) = routed("plane_board.kicad_pcb", variant, 0);
    let (_, after) = routed("plane_board.kicad_pcb", variant, 2);

    assert_ne!(
        after.cmp_rank(&before),
        std::cmp::Ordering::Greater,
        "repair handed back {after:?} for {before:?}"
    );
}

/// On `plane_board` repair finds nothing ahead of the winner, and the winner
/// comes out of the ranking exactly as it was routed.
#[test]
fn a_winner_repair_cannot_improve_is_left_as_it_was() {
    let (mut world, library) = board("plane_board.kicad_pcb");
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let rules = ruleset_for_world(preset, &world);
    let design_rules = DesignRules::from_constraints(&preset.constraints());

    let results = generate_variants(
        &mut world,
        &library,
        &rules,
        &design_rules,
        &default_variant_configs(),
    );
    let winner = &results[0];
    assert!(
        !winner.repaired,
        "{} won repaired, but repair was expected to find nothing here",
        winner.name
    );

    let (alone, _) = routed("plane_board.kicad_pcb", &winner.name, 0);
    assert!(
        winner.routes == alone.routes && winner.vias == alone.vias,
        "the winner is not the copper its own settings route"
    );
}
