//! Does routing the same board twice give the same board?
//!
//! `cargo test --release -p cypcb-autoroute --test is_the_router_repeatable -- --ignored --nocapture`
//!
//! Every number in `docs/routing.md` is one run of the router, and every
//! comparison in it - fourteen dropped instruments, two sweeps, five ratchets -
//! assumes that re-running with the same settings gives the same answer. If it
//! does not, each of those differences carries an error bar nobody has written
//! down, and the "noise band" recorded against price is measuring the router as
//! much as the price.
//!
//! `qfp_fanout` made the question urgent: its band across prices 0.22..0.28 is
//! 102 violations wide, a third of the value it guards, against 38 and 30 on
//! the two older dense boards.
//!
//! This routes each board three times with one config and compares. Rust
//! randomises `HashMap` iteration order per process, so anything that orders
//! work by walking a map produces a different board each run - which is the
//! first thing to rule out.

use cypcb_autoroute::pathfinder_v2::PathFinderStrategy;
use cypcb_autoroute::scoring::{score_board, ScoreWeights};
use cypcb_autoroute::strategy::RoutingStrategy;
use cypcb_autoroute::AutorouteConfig;
use cypcb_drc::presets::DesignRules;
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

use std::path::{Path, PathBuf};

const RUNS: usize = 3;

fn fixture_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// What one run of the router produced.
#[derive(PartialEq, Eq, Debug)]
struct Run {
    violations: usize,
    shorts: usize,
    segments: usize,
    vias: usize,
    /// Total copper in nanometres, which catches a board that scores the same
    /// by a different path.
    length_nm: i64,
}

fn route_once(filename: &str) -> Run {
    let parsed = parse_kicad_pcb(&fixture_path(filename))
        .unwrap_or_else(|e| panic!("failed to parse {filename}: {e:?}"));
    let mut world = parsed.world;
    let library = parsed.library;
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").unwrap());
    let config = AutorouteConfig::default();

    let result = PathFinderStrategy.route(&mut world, &library, &rules, &config);
    let segments = result.routes.len();
    let vias = result.vias.len();

    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);
    let score = score_board(
        &mut world,
        &DesignRules::jlcpcb_2layer(),
        &ScoreWeights::default(),
    );

    Run {
        violations: score.drc_violations as usize,
        shorts: score.shorts as usize,
        segments,
        vias,
        length_nm: score.total_length.raw(),
    }
}

#[test]
#[ignore = "slow: routes every fixture three times"]
fn the_same_board_routed_twice_is_the_same_board() {
    let mut unstable = Vec::new();

    eprintln!();
    eprintln!("Three runs of the same config, per board:");
    eprintln!();

    for benchmark in BENCHMARKS {
        let runs: Vec<Run> = (0..RUNS).map(|_| route_once(benchmark.filename)).collect();

        let identical = runs.iter().all(|r| *r == runs[0]);
        eprintln!(
            "  {:<24} {}",
            benchmark.filename.trim_end_matches(".kicad_pcb"),
            if identical { "identical" } else { "DIFFERENT" }
        );
        for (index, run) in runs.iter().enumerate() {
            eprintln!(
                "      run {}: {:>4} violations, {:>4} shorts, {:>5} segments, {:>4} vias, {:>10} nm",
                index + 1,
                run.violations,
                run.shorts,
                run.segments,
                run.vias,
                run.length_nm
            );
        }

        if !identical {
            unstable.push(benchmark.filename);
        }
    }

    eprintln!();
    if unstable.is_empty() {
        eprintln!("Every board routed the same way {RUNS} times. The bands recorded against");
        eprintln!("price in docs/routing.md are the price, not the run.");
    } else {
        eprintln!("Boards that did not repeat: {}", unstable.join(", "));
        eprintln!("Every measurement in docs/routing.md taken on these carries an error bar");
        eprintln!("nobody has written down.");
    }

    assert!(
        unstable.is_empty(),
        "routing the same board with the same settings has to give the same board; \
         these did not: {}",
        unstable.join(", ")
    );
}
