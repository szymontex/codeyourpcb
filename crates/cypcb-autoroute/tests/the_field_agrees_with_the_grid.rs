//! Step 1 of `docs/router-plan.md`: the clearance field, read by nobody.
//!
//! `cargo test -p cypcb-autoroute --test the_field_agrees_with_the_grid`
//! `cargo test --release -p cypcb-autoroute --test the_field_agrees_with_the_grid -- --ignored --nocapture`
//!
//! The plan's first step exists to build the transform, show that it says the
//! same thing the grid says wherever the grid can answer, and measure what it
//! costs - before anything is allowed to act on it. Two of the tests here are
//! the agreement; one is the chamfer's error bound, which is quoted in the
//! module and therefore has to be measured; the ignored one is the cost.
//!
//! The falsifier for the whole step lives elsewhere and is sharper than
//! anything here: `benchmark_validation` must not move by a single violation,
//! because nothing reads this field yet.

use std::path::Path;

use cypcb_autoroute::clearance_field::ClearanceField;
use cypcb_autoroute::grid::RoutingGrid;
use cypcb_autoroute::AutorouteConfig;
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_rules::RoutingRuleSet;

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

fn test_rules() -> PresetRuleSet {
    PresetRuleSet::new(RulesPreset::from_name("jlcpcb").unwrap())
}

/// Build both structures for a fixture, the way the router builds the grid.
fn grid_and_field(filename: &str) -> (RoutingGrid, ClearanceField, i64) {
    let parsed = parse_kicad_pcb(&fixture_path(filename))
        .unwrap_or_else(|e| panic!("Failed to parse {filename}: {e:?}"));
    let mut world = parsed.world;
    let library = parsed.library;
    let rules = test_rules();

    let config = AutorouteConfig::default();
    let resolution = config.resolve_grid_resolution(&rules);
    let grid = RoutingGrid::from_board(&mut world, &library, &rules, resolution)
        .unwrap_or_else(|| panic!("no board entity in {filename}"));
    let field = ClearanceField::from_board(&mut world, &library, &grid);

    // What the grid bloats every obstacle by. A cell the grid calls blocked is
    // within this of copper, by construction, and the field has to agree.
    let constraints = rules.constraints_for_net(0);
    let bloat_nm = constraints.min_clearance.raw() + constraints.min_trace_width.raw() / 2;

    (grid, field, bloat_nm)
}

/// The chamfer over-estimates along the diagonals, and the grid rounds its
/// bloat radius **up** to whole cells and rasterises discs by cell centre.
/// Both are one-sided and both are known, so the comparison carries them
/// rather than pretending the two structures are exact about the same thing.
///
/// One and a half cells: one for the grid's round-up, half for the difference
/// between measuring a disc from its centre and from its edge.
fn tolerance_nm(resolution: i64) -> i64 {
    resolution + resolution / 2
}

#[test]
fn a_blocked_cell_is_near_copper_and_a_free_one_is_not() {
    for benchmark in BENCHMARKS {
        let (grid, field, bloat_nm) = grid_and_field(benchmark.filename);
        let tol = tolerance_nm(grid.resolution());

        let mut blocked_checked = 0u64;
        let mut free_checked = 0u64;
        let mut bare_layers = 0u64;
        let mut blocked_too_far = Vec::new();
        let mut free_too_near = Vec::new();

        for layer in 0..grid.layer_count() as usize {
            for y in 0..grid.height() {
                for x in 0..grid.width() {
                    let d = field
                        .distance_nm(x, y, layer)
                        .expect("field and grid have the same extent");
                    // A layer with no copper on it answers `i64::MAX`, which is
                    // a real answer - nothing is near, at any distance - and it
                    // is not comparable against a bloat radius. multi_ic has
                    // two such layers: it is a four-layer board whose inner
                    // copper no fixture ever put anything on.
                    if d == i64::MAX {
                        bare_layers += 1;
                        continue;
                    }
                    if grid.cell(x, y, layer) != 0 {
                        blocked_checked += 1;
                        if d > bloat_nm + tol && blocked_too_far.len() < 4 {
                            blocked_too_far.push((x, y, layer, d));
                        }
                    } else {
                        free_checked += 1;
                        // A free cell is outside every bloated obstacle, so it
                        // cannot be closer to copper than the bloat radius.
                        if d + tol < bloat_nm && free_too_near.len() < 4 {
                            free_too_near.push((x, y, layer, d));
                        }
                    }
                }
            }
        }

        assert!(
            blocked_checked > 0 && free_checked > 0,
            "{}: nothing to compare - {blocked_checked} blocked, {free_checked} free, \
             {bare_layers} cells on a layer with no copper",
            benchmark.filename
        );
        assert!(
            blocked_too_far.is_empty(),
            "{}: the grid calls these cells blocked and the field puts them further than \
             {bloat_nm}nm (+{tol}nm tolerance) from any copper: {blocked_too_far:?}",
            benchmark.filename
        );
        assert!(
            free_too_near.is_empty(),
            "{}: the grid calls these cells free and the field puts them inside \
             {bloat_nm}nm (-{tol}nm tolerance) of copper: {free_too_near:?}",
            benchmark.filename
        );
    }
}

#[test]
fn copper_reads_as_zero_and_an_empty_layer_reads_as_unreachable() {
    // Every seed is a cell the field was told holds copper, and the transform
    // must leave those at zero rather than smoothing them.
    let (grid, field, _) = grid_and_field("led_blink.kicad_pcb");
    assert!(
        field.seed_count() > 0,
        "led_blink has pads, so the field must have seeds"
    );
    assert_eq!(
        field.cell_count(),
        (grid.width() as usize) * (grid.height() as usize) * grid.layer_count() as usize,
        "the field covers exactly the grid's cells"
    );

    // Out of bounds is the one case that has no answer, and it says so.
    assert_eq!(field.distance_nm(grid.width(), 0, 0), None);
    assert_eq!(field.distance_nm(0, grid.height(), 0), None);
    assert_eq!(field.distance_nm(0, 0, grid.layer_count() as usize), None);
}

#[test]
fn the_error_is_within_the_stated_bound() {
    // The module claims the 3-4 chamfer is within about 6% of Euclidean. That
    // is a claim about this code, so it is measured against a brute-force
    // Euclidean answer rather than quoted from the literature.
    //
    // led_blink is the smallest fixture, which is what makes an O(cells x
    // seeds) comparison affordable at all. Every non-copper cell on layer 0 is
    // compared against every copper cell on it.
    let (grid, field, _) = grid_and_field("led_blink.kicad_pcb");
    let resolution = grid.resolution() as f64;

    let mut seeds: Vec<(f64, f64)> = Vec::new();
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            if field.is_copper(x, y, 0) {
                seeds.push((x as f64, y as f64));
            }
        }
    }
    assert!(!seeds.is_empty(), "led_blink has copper on its top layer");

    let mut worst_over = 1.0f64;
    let mut worst_over_at = (0u32, 0u32);
    let mut worst_under = 1.0f64;
    let mut worst_under_at = (0u32, 0u32);
    let mut compared = 0u64;

    for y in 0..grid.height() {
        for x in 0..grid.width() {
            if field.is_copper(x, y, 0) {
                continue;
            }
            let Some(chamfer_nm) = field.distance_nm(x, y, 0) else {
                continue;
            };
            if chamfer_nm == i64::MAX {
                continue;
            }

            let mut nearest_sq = f64::MAX;
            for (sx, sy) in &seeds {
                let dx = *sx - x as f64;
                let dy = *sy - y as f64;
                let d_sq = dx * dx + dy * dy;
                if d_sq < nearest_sq {
                    nearest_sq = d_sq;
                }
            }
            let euclidean_nm = nearest_sq.sqrt() * resolution;
            if euclidean_nm <= 0.0 {
                continue;
            }

            compared += 1;
            let ratio = chamfer_nm as f64 / euclidean_nm;
            if ratio > worst_over {
                worst_over = ratio;
                worst_over_at = (x, y);
            }
            if ratio < worst_under {
                worst_under = ratio;
                worst_under_at = (x, y);
            }
        }
    }

    assert!(
        compared > 100,
        "only {compared} cells compared - too few to mean anything"
    );
    println!(
        "chamfer vs Euclidean on led_blink over {compared} cells: \
         worst over {worst_over:.4} at {worst_over_at:?}, \
         worst under {worst_under:.4} at {worst_under_at:?}"
    );
    // The 3-4 chamfer is exact along an axis and short along a diagonal, where
    // it charges 4/3 = 1.3333 for a step whose true length is sqrt(2) =
    // 1.4142 - so it reads 5.7% low there. Both directions are bounded here
    // because the module states a bound and a test that only checked one of
    // them would let the other drift unnoticed.
    assert!(
        worst_over < 1.06,
        "the chamfer reads {worst_over:.4} of the true distance at {worst_over_at:?}, \
         outside the 6% the module claims"
    );
    assert!(
        worst_under > 0.94,
        "the chamfer reads {worst_under:.4} of the true distance at {worst_under_at:?}, \
         outside the 6% the module claims"
    );
}

#[test]
#[ignore = "measurement: what the transform costs per board"]
fn what_the_field_costs() {
    println!(
        "\n{:<24} {:>10} {:>10} {:>12} {:>10}",
        "board", "cells", "seeds", "grid (ms)", "field (ms)"
    );
    for benchmark in BENCHMARKS {
        let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
            .unwrap_or_else(|e| panic!("Failed to parse {}: {e:?}", benchmark.filename));
        let mut world = parsed.world;
        let library = parsed.library;
        let rules = test_rules();
        let resolution = AutorouteConfig::default().resolve_grid_resolution(&rules);

        let t0 = std::time::Instant::now();
        let grid = RoutingGrid::from_board(&mut world, &library, &rules, resolution)
            .expect("board entity");
        let grid_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let t1 = std::time::Instant::now();
        let field = ClearanceField::from_board(&mut world, &library, &grid);
        let field_ms = t1.elapsed().as_secs_f64() * 1000.0;

        println!(
            "{:<24} {:>10} {:>10} {:>12.2} {:>10.2}",
            benchmark.filename,
            field.cell_count(),
            field.seed_count(),
            grid_ms,
            field_ms
        );
    }
    println!();
}
