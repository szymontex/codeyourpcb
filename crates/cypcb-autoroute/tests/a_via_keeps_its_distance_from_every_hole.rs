//! A via priced by the holes too close to it, on the board it was measured on.
//!
//! `cargo test --release -p cypcb-autoroute --test a_via_keeps_its_distance_from_every_hole -- --nocapture`
//!
//! The unit tests say the map counts the right cells. This says the search
//! reads them: the price changes where vias go on a real board, and the board
//! it was measured on comes out with fewer shorts for it.

use std::path::Path;

use cypcb_autoroute::via_optimizer::BoardObstacles;
use cypcb_autoroute::{route_board, AutorouteConfig, AutorouteParams};
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, shorts, DesignRules};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// How far a point is from the path of a hole's bit, in nm.
fn distance_to_path(at: [i64; 2], start: [i64; 2], end: [i64; 2]) -> f64 {
    let (px, py) = ((at[0] - start[0]) as f64, (at[1] - start[1]) as f64);
    let (dx, dy) = ((end[0] - start[0]) as f64, (end[1] - start[1]) as f64);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        ((px * dx + py * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((px - t * dx).powi(2) + (py - t * dy).powi(2)).sqrt()
}

/// Route `stm32_breakout` the way the `High-Density` variants do, at one
/// price, and return the shorts, every via's position, and how many vias sit
/// closer to a hole the board had before routing than the rule allows.
fn route(price: f64) -> (usize, Vec<(i64, i64)>, usize) {
    let parsed = parse_kicad_pcb(&fixture_path("stm32_breakout.kicad_pcb"))
        .unwrap_or_else(|e| panic!("failed to parse stm32_breakout: {e:?}"));
    let mut world = parsed.world;
    let library = parsed.library;
    let preset = preset_for_world(
        cypcb_rules::presets::RulesPreset::JlcpcbStandard2Layer,
        &world,
    );
    let rules = ruleset_for_world(preset, &world);
    let config = AutorouteConfig {
        params: AutorouteParams {
            density: 1.5,
            ..AutorouteParams::default()
        },
        repair_passes: 0,
        via_near_hole_penalty: price,
        ..AutorouteConfig::default()
    };

    let board_holes: Vec<_> = BoardObstacles::from_board(&mut world, &library)
        .holes()
        .collect();
    let spacing = preset.constraints().min_hole_to_hole.raw();
    let result = route_board(&mut world, &library, &rules, &config);
    let too_close = result
        .vias
        .iter()
        .filter(|via| {
            let at = [via.position.x.raw(), via.position.y.raw()];
            board_holes.iter().any(|&(start, end, radius)| {
                distance_to_path(at, start, end) < (via.drill.raw() / 2 + radius + spacing) as f64
            })
        })
        .count();
    let mut vias: Vec<(i64, i64)> = result
        .vias
        .iter()
        .map(|via| (via.position.x.raw(), via.position.y.raw()))
        .collect();
    vias.sort_unstable();
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let report = run_drc(
        &mut world,
        &DesignRules::from_constraints(&preset.constraints()),
    );
    (shorts(&report.violations), vias, too_close)
}

#[test]
fn the_price_moves_the_vias_and_takes_shorts_off_the_board_it_was_measured_on() {
    let (shorts_before, vias_before, close_before) = route(0.0);
    let (shorts_after, vias_after, close_after) = route(5.0);
    println!(
        "stm32_breakout High-Density: shorts {shorts_before} -> {shorts_after}, vias {} -> {}, \
         vias too close to a board hole {close_before} -> {close_after}",
        vias_before.len(),
        vias_after.len()
    );
    assert_ne!(vias_before, vias_after, "the price moved no via");
    assert!(
        shorts_after < shorts_before,
        "shorts {shorts_before} -> {shorts_after}"
    );
    // Measured 0 when the search reads the pins and the designer's vias, and
    // 1 when it prices only the holes it drills itself.
    assert_eq!(
        close_after, 0,
        "a via landed too close to a hole the board already had"
    );
}
