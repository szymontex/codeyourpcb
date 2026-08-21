//! What the repair pass decided, in its own words.
//!
//! `cargo test -p cypcb-autoroute --test repair_report -- --ignored --nocapture`
//!
//! `repair_routes` narrates every attempt through `tracing`, and no test ever
//! turned a subscriber on, so the pass has been running blind: the benchmark
//! numbers are identical with and without it and nothing said why. This turns
//! the log on for one fixture and prints it.

use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, DesignRules};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;
use cypcb_rules::presets::RulesPreset;

#[test]
#[ignore = "diagnostic: prints the repair pass's own decisions"]
fn what_repair_decided() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .with_test_writer()
        .init();

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark/multi_ic.kicad_pcb");

    let parsed = parse_kicad_pcb(&fixture).expect("the fixture parses");
    let library = parsed.library;

    // `multi_ic` has four copper layers, so `cypcb check` reads it against
    // `jlcpcb_standard_4layer`. A fixed two-layer table here would report on a
    // board nobody ships.
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &parsed.world);
    let rules = ruleset_for_world(preset, &parsed.world);
    let drc_rules = DesignRules::from_constraints(&preset.constraints());
    eprintln!("graded on {}", preset.name());

    // What the pass costs, against what it returns. Each repair attempt is a
    // full re-route of the board, and the default is two radii of two passes.
    let off = AutorouteConfig::default();
    let on = AutorouteConfig {
        repair_passes: 2,
        ..AutorouteConfig::default()
    };

    for (label, config) in [("repair off", &off), ("repair on ", &on)] {
        let mut board = parse_kicad_pcb(&fixture).expect("the fixture parses").world;
        let started = std::time::Instant::now();
        let result = route_board(&mut board, &library, &rules, config);
        let elapsed = started.elapsed();

        apply_routes(&mut board, &result);
        board.rebuild_spatial_index_from_library(&library);
        let drc = run_drc(&mut board, &drc_rules);

        eprintln!(
            "{label}: {:?}, {} routes, {} violations, {:.1}s",
            result.status,
            result.route_count(),
            drc.violations.len(),
            elapsed.as_secs_f64()
        );
    }
}
