//! A via priced by the holes too close to it, on the board it was measured on.
//!
//! `cargo test --release -p cypcb-autoroute --test a_via_keeps_its_distance_from_every_hole -- --nocapture`
//!
//! The unit tests say the map counts the right cells. This says the search
//! reads them: the price changes where vias go on a real board, and no via
//! the priced search places sits too close to a hole the board already had.
//!
//! What it claimed until 2026-09-25: the board the price was measured on,
//! `stm32_breakout`, comes out with fewer shorts for it. That was measured on
//! the boards as the KiCad reader then read them - a mirror image of the
//! files. Read the right way up, the price does not take shorts off
//! `stm32_breakout`. Measured on every benchmark board, High-Density with no
//! repair pass, price 0 then 5:
//!
//! | board          | shorts    | vias       | too close to a hole |
//! |----------------|-----------|------------|---------------------|
//! | led_blink      | 0 -> 0    | 4 -> 4     | 0 -> 0              |
//! | stm32_breakout | 38 -> 44  | 155 -> 163 | 0 -> 0              |
//! | multi_ic       | 74 -> 64  | 193 -> 201 | 0 -> 0              |
//! | shift_driver   | 8 -> 9    | 91 -> 92   | 5 -> 0              |
//! | qfp_fanout     | 68 -> 101 | 207 -> 230 | 0 -> 0              |
//! | plane_board    | 4 -> 4    | 39 -> 39   | 0 -> 0              |
//!
//! What it claims now: the price does its own job - on `shift_driver` it
//! takes all five vias off the holes they crowded - and it takes shorts off
//! one board, `multi_ic`, while adding them on three. The list of boards it
//! helps is exact, so the day it changes this test is read again.

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

/// Route a benchmark board the way the `High-Density` variants do, at one
/// price, and return the shorts, every via's position, and how many vias sit
/// closer to a hole the board had before routing than the rule allows.
fn route(board: &str, price: f64) -> (usize, Vec<(i64, i64)>, usize) {
    let parsed = parse_kicad_pcb(&fixture_path(board))
        .unwrap_or_else(|e| panic!("failed to parse {board}: {e:?}"));
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
fn the_price_moves_the_vias_and_keeps_them_off_the_holes() {
    let mut helped = Vec::new();
    for board in [
        "led_blink.kicad_pcb",
        "stm32_breakout.kicad_pcb",
        "multi_ic.kicad_pcb",
        "shift_driver.kicad_pcb",
        "qfp_fanout.kicad_pcb",
        "plane_board.kicad_pcb",
    ] {
        let (shorts_before, vias_before, close_before) = route(board, 0.0);
        let (shorts_after, vias_after, close_after) = route(board, 5.0);
        println!(
            "{board} High-Density: shorts {shorts_before} -> {shorts_after}, vias {} -> {}, \
             vias too close to a board hole {close_before} -> {close_after}",
            vias_before.len(),
            vias_after.len()
        );
        if board == "stm32_breakout.kicad_pcb" {
            assert_ne!(vias_before, vias_after, "the price moved no via");
        }
        // Measured 0 when the search reads the pins and the designer's vias,
        // and 1 when it prices only the holes it drills itself.
        assert_eq!(
            close_after, 0,
            "{board}: a via landed too close to a hole the board already had"
        );
        if shorts_after < shorts_before {
            helped.push(board);
        }
    }
    assert_eq!(
        helped,
        ["multi_ic.kicad_pcb"],
        "the boards the price takes shorts off; the table in this file's \
         header is the measurement this list comes from"
    );
}
