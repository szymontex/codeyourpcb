//! How many rows the checker prints for one piece of copper touching another.
//!
//! `cargo test --release -p cypcb-autoroute --test how_many_rows_one_contact_gets -- --ignored --nocapture`
//!
//! `ClearanceRule` reports per pair of *segments*, and a trace is a polyline:
//! two traces running beside each other for 10mm are one contact and as many
//! rows as they have segments in that stretch. Nothing had counted the
//! difference until a neck experiment made it visible - necking cut two extra
//! segments per run and the violation count rose by 171 on `multi_ic` while
//! the set of feature pairs in fault did not change at all.
//!
//! **Every count this project publishes is a count of rows.** The ratchets in
//! `benchmark_validation`, the noise bands in `cypcb_autoroute::noise_band`,
//! every sweep table in `docs/routing.md`. If a rule reported each pair once
//! at its worst point they would all move together, so the size of the
//! difference is worth knowing before anybody decides whether to make it.
//!
//! This measures the shipped configuration - the default router, each board on
//! the fab table its own layer count asks for - so the numbers are about the
//! boards this project actually publishes.

use std::collections::BTreeMap;
use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, DesignRules, ViolationKind};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;
use cypcb_rules::presets::RulesPreset;

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// The two features a violation is about, as the message names them.
///
/// `U1 ↔ trace 'GND': Clearance violation: ...` - everything before the colon
/// is the pair, and it is the same string however many segments report it.
fn pair_of(message: &str) -> String {
    message
        .split_once(':')
        .map(|(pair, _)| pair.trim().to_string())
        .unwrap_or_else(|| message.to_string())
}

#[test]
#[ignore = "diagnostic: rows per contact, per board, on the shipped router"]
fn how_many_rows_one_contact_gets() {
    eprintln!();
    eprintln!(
        "{:<18} {:>7} {:>7} {:>9}   worst offender",
        "board", "rows", "pairs", "rows/pair"
    );

    let mut total_rows = 0usize;
    let mut total_pairs = 0usize;

    for benchmark in cypcb_kicad::BENCHMARKS {
        let parsed =
            parse_kicad_pcb(&fixture_path(benchmark.filename)).expect("the fixture parses");
        let mut world = parsed.world;
        let library = parsed.library;

        let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
        let rules = ruleset_for_world(preset, &world);
        let drc_rules = DesignRules::from_constraints(&preset.constraints());

        let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
        apply_routes(&mut world, &result);
        world.rebuild_spatial_index_from_library(&library);

        let drc = run_drc(&mut world, &drc_rules);
        let mut rows_per_pair: BTreeMap<String, usize> = BTreeMap::new();
        for violation in &drc.violations {
            if violation.kind != ViolationKind::Clearance {
                continue;
            }
            *rows_per_pair
                .entry(pair_of(&violation.message))
                .or_insert(0) += 1;
        }

        let rows: usize = rows_per_pair.values().sum();
        let pairs = rows_per_pair.len();
        total_rows += rows;
        total_pairs += pairs;

        let worst = rows_per_pair
            .iter()
            .max_by_key(|(_, count)| **count)
            .map(|(pair, count)| format!("{count} rows for {pair}"))
            .unwrap_or_else(|| "nothing in fault".to_string());

        eprintln!(
            "{:<18} {:>7} {:>7} {:>9.2}   {}",
            benchmark.filename.trim_end_matches(".kicad_pcb"),
            rows,
            pairs,
            if pairs == 0 {
                0.0
            } else {
                rows as f64 / pairs as f64
            },
            worst
        );
    }

    eprintln!();
    eprintln!(
        "across the six: {} rows for {} contacts, {:.2} rows each",
        total_rows,
        total_pairs,
        total_rows as f64 / total_pairs as f64
    );
    eprintln!();
    eprintln!("Only `clearance` is counted. The other kinds report per feature");
    eprintln!("rather than per pair of segments and do not multiply this way.");
}
