//! Which connections the router gives up on, by name.
//!
//! `cargo test -p cypcb-autoroute --test abandoned_connections -- --ignored --nocapture`
//!
//! `RoutingResult::Partial` carries a count. A count is not something anyone
//! can act on: "6 unrouted" says a board is incomplete without saying which
//! six nets, which pads, or what stood in the way. The router logs each
//! abandoned connection through `tracing`; this turns the subscriber on and
//! routes every fixture so the names come out.

use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

#[test]
#[ignore = "diagnostic: names every connection the router abandons"]
fn what_the_router_gave_up_on() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_target(false)
        .without_time()
        .with_test_writer()
        .init();

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").unwrap());

    for benchmark in BENCHMARKS {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures/benchmark")
            .join(benchmark.filename);

        let parsed = parse_kicad_pcb(&fixture)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
        let mut world = parsed.world;

        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);

        let result = route_board(
            &mut world,
            &parsed.library,
            &rules,
            &AutorouteConfig::default(),
        );

        eprintln!("  {:?}, {} routes", result.status, result.route_count());
    }
}
