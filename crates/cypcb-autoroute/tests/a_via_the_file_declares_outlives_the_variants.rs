//! Does ranking the variants leave the vias the file declares on the board?
//!
//! `cargo test -p cypcb-autoroute --test a_via_the_file_declares_outlives_the_variants -- --nocapture`
//!
//! `generate_variants` clears the board before each variant and again before
//! it applies the winner. The clear asked `!via.locked` and nothing about who
//! put the via there, so it deleted every via the file declares: each variant
//! was routed and scored on a board the designer never drew, and the winner
//! was applied to it. `apply_routes` had already stopped doing this - it asks
//! for `RouterPlaced` - and this clear was the copy that kept the old question.
//!
//! Measured before the fix over the 37 boards the ranking probes use: two
//! boards declare vias, and both lost all of them. `blind-via` declares 2 and
//! every one of its 13 variants scored 2002 lower than the same routes score
//! on the board as written; `stitched-plane` declares 16 by a stitch rule and
//! scored 16 lower. The winner did not change on either board, so the ranking
//! was right by luck and the board handed back was not.

use std::path::Path;

use cypcb_autoroute::scoring::{score_board, ScoreWeights};
use cypcb_autoroute::variant::{default_variant_configs, generate_variants};
use cypcb_drc::{preset_for_world, ruleset_for_world, DesignRules};
use cypcb_router::apply_routes;
use cypcb_router::types::RoutingResult;
use cypcb_rules::presets::RulesPreset;
use cypcb_world::components::trace::{RouterPlaced, Via};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn load(example: &str) -> (BoardWorld, FootprintLibrary) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(example);
    let source = std::fs::read_to_string(&path).expect("the example exists");
    let parsed = cypcb_parser::parse(&source);
    assert!(parsed.errors.is_empty(), "{example} parses");
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let _ = sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);
    (world, library)
}

fn rebuild(world: &mut BoardWorld, library: &FootprintLibrary) {
    world.rebuild_spatial_index_with_traces(|name| {
        library.get(name).map(|fp| fp.courtyard).unwrap_or_else(|| {
            cypcb_core::Rect::from_center_size(
                cypcb_core::Point::ORIGIN,
                (cypcb_core::Nm::from_mm(1.0), cypcb_core::Nm::from_mm(1.0)),
            )
        })
    });
}

/// Vias on the board that the router did not put there.
fn vias_not_the_routers(world: &mut BoardWorld) -> usize {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&Via, Option<&RouterPlaced>)>();
    query
        .iter(ecs)
        .filter(|(_, placed)| placed.is_none())
        .count()
}

fn check(example: &str, declared: usize) {
    let (mut world, library) = load(example);
    assert_eq!(
        vias_not_the_routers(&mut world),
        declared,
        "{example} declares {declared} vias; if that changed, this test \
         no longer measures what it says"
    );

    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let rules = ruleset_for_world(preset, &world);
    let design_rules = DesignRules::from_constraints(&preset.constraints());
    let variants = generate_variants(
        &mut world,
        &library,
        &rules,
        &design_rules,
        &default_variant_configs(),
    );
    let winner = variants.first().expect("at least one variant routes");

    let kept = vias_not_the_routers(&mut world);
    println!("{example}: {declared} vias declared, {kept} on the board after ranking");
    assert_eq!(
        kept, declared,
        "ranking the variants deleted a via the file declares"
    );

    // The winner's score has to be the score of its routes on the board as
    // written - which is what a fresh load plus those routes is.
    let (mut fresh, fresh_library) = load(example);
    apply_routes(
        &mut fresh,
        &RoutingResult::complete(winner.routes.clone(), winner.vias.clone()),
    );
    rebuild(&mut fresh, &fresh_library);
    let as_written = score_board(&mut fresh, &design_rules, &ScoreWeights::default());
    println!(
        "{example}: winner [{}] composite {:.2} while ranked, {:.2} on the board as written",
        winner.name, winner.score.composite, as_written.composite
    );
    assert!(
        (winner.score.composite - as_written.composite).abs() < 1e-6,
        "the winner was scored on a different board than the one the file describes"
    );
}

#[test]
fn a_via_on_a_trace_survives_ranking_and_counts_in_the_score() {
    check("blind-via.cypcb", 2);
}

#[test]
fn a_via_from_a_stitch_rule_survives_ranking_and_counts_in_the_score() {
    check("stitched-plane.cypcb", 16);
}
