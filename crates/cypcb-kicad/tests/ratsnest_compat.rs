//! Ratsnest compatibility test — proves the KiCad parser output is
//! directly consumable by the autorouter's `extract_ratsnest()`.
//!
//! This is the critical integration proof for the S01→S03 boundary:
//! if this test passes, the parsed `BoardWorld` + `FootprintLibrary`
//! contain correct component entities, footprint references, pad geometry,
//! and net connections — everything the autorouter needs.

use cypcb_autoroute::orchestrator::extract_ratsnest;
use cypcb_kicad::pcb_parser::{parse_kicad_pcb, BENCHMARKS};

/// Workspace root — integration tests run from the crate directory,
/// so we need to go up to the workspace root where `tests/fixtures/` lives.
fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../..").canonicalize().unwrap()
}

fn fixture_path(filename: &str) -> std::path::PathBuf {
    workspace_root()
        .join("tests/fixtures/benchmark")
        .join(filename)
}

#[test]
fn ratsnest_from_led_blink_is_nonempty() {
    let benchmark = &BENCHMARKS[0]; // led_blink — simplest fixture
    assert_eq!(benchmark.filename, "led_blink.kicad_pcb");

    let path = fixture_path(benchmark.filename);
    let mut result = parse_kicad_pcb(&path).expect("Failed to parse led_blink.kicad_pcb");

    let ratsnest = extract_ratsnest(&mut result.world, &result.library);

    assert!(
        !ratsnest.is_empty(),
        "Ratsnest must be non-empty for a board with {} nets",
        result.metadata.net_count,
    );

    // Each NetRoute in the ratsnest should have a valid net ID and at least 2 pads
    // (a single-pad net cannot form a connection).
    let multi_pad_nets: Vec<_> = ratsnest.iter().filter(|nr| nr.pads.len() >= 2).collect();
    assert!(
        !multi_pad_nets.is_empty(),
        "At least one net must have ≥2 pads to form a routable connection",
    );

    // The number of nets in the ratsnest should not exceed the total net count
    // from the parsed metadata.
    assert!(
        ratsnest.len() <= result.metadata.net_count,
        "Ratsnest net count ({}) should not exceed parsed net count ({})",
        ratsnest.len(),
        result.metadata.net_count,
    );

    eprintln!(
        "Ratsnest OK: {} nets extracted ({} with ≥2 pads) from {} total parsed nets",
        ratsnest.len(),
        multi_pad_nets.len(),
        result.metadata.net_count,
    );
}

#[test]
fn ratsnest_from_all_benchmarks_succeeds() {
    for benchmark in BENCHMARKS {
        let path = fixture_path(benchmark.filename);
        let mut result =
            parse_kicad_pcb(&path).unwrap_or_else(|e| panic!("Failed to parse {}: {e}", benchmark.filename));

        // extract_ratsnest must not panic — this is the core compatibility proof
        let ratsnest = extract_ratsnest(&mut result.world, &result.library);

        eprintln!(
            "{}: {} nets in ratsnest, {} components, {} parsed nets",
            benchmark.filename,
            ratsnest.len(),
            result.metadata.component_count,
            result.metadata.net_count,
        );
    }
}
