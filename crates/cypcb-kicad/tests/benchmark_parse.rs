//! Integration tests for benchmark fixture parsing.
//!
//! Validates that all 3 benchmark `.kicad_pcb` files parse successfully
//! and produce metadata within expected ranges. These tests exercise the
//! parser against realistic KiCad 8 files of varying complexity.

use cypcb_kicad::pcb_parser::{get_benchmarks, parse_kicad_pcb, BenchmarkComplexity, BENCHMARKS};

/// Workspace root — integration tests run from the crate directory,
/// so we need to go up to the workspace root where `tests/fixtures/` lives.
fn workspace_root() -> std::path::PathBuf {
    // crates/cypcb-kicad/ → ../../
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../..").canonicalize().unwrap()
}

fn fixture_path(filename: &str) -> std::path::PathBuf {
    workspace_root()
        .join("tests/fixtures/benchmark")
        .join(filename)
}

// ---------------------------------------------------------------------------
// Individual benchmark tests
// ---------------------------------------------------------------------------

#[test]
fn test_parse_led_blink() {
    let path = fixture_path("led_blink.kicad_pcb");
    let result = parse_kicad_pcb(&path).expect("Failed to parse led_blink.kicad_pcb");
    let m = &result.metadata;

    assert_eq!(m.version, 20240108, "Expected KiCad 8 version");
    assert_eq!(m.component_count, 7, "Expected 7 components");
    assert_eq!(m.net_count, 7, "Expected 7 nets");
    assert_eq!(m.layer_count, 2, "Expected 2-layer board");

    // Board size: 40x30mm
    assert!(m.board_size_mm.0 > 0.0, "Board width must be non-zero");
    assert!(m.board_size_mm.1 > 0.0, "Board height must be non-zero");
    assert!(
        (m.board_size_mm.0 - 40.0).abs() < 1.0,
        "Board width ~40mm, got {}",
        m.board_size_mm.0
    );
    assert!(
        (m.board_size_mm.1 - 30.0).abs() < 1.0,
        "Board height ~30mm, got {}",
        m.board_size_mm.1
    );

    // Traces and vias
    assert_eq!(m.trace_segment_count, 3, "Expected 3 trace segments");
    assert_eq!(m.via_count, 2, "Expected 2 vias");

    // Library must have entries
    assert!(!result.library.is_empty(), "Library must have footprints");

    // World must have nets
    assert!(result.world.net_count() > 0, "World must have nets");
}

#[test]
fn test_parse_stm32_breakout() {
    let path = fixture_path("stm32_breakout.kicad_pcb");
    let result = parse_kicad_pcb(&path).expect("Failed to parse stm32_breakout.kicad_pcb");
    let m = &result.metadata;

    assert_eq!(m.version, 20240108, "Expected KiCad 8 version");
    assert_eq!(m.layer_count, 2, "Expected 2-layer board");

    // ±20% tolerance for medium complexity
    let expected_comps = 29usize;
    let expected_nets = 40usize;
    assert_within_tolerance(m.component_count, expected_comps, 0.20, "component_count");
    assert_within_tolerance(m.net_count, expected_nets, 0.20, "net_count");

    // Board size non-zero
    assert!(m.board_size_mm.0 > 0.0, "Board width must be non-zero");
    assert!(m.board_size_mm.1 > 0.0, "Board height must be non-zero");

    // Library and world
    assert!(!result.library.is_empty(), "Library must have footprints");
    assert!(result.world.net_count() > 0, "World must have nets");

    // Reference routes present
    assert!(
        result.reference_routes.is_some(),
        "Expected reference routes"
    );
}

#[test]
fn test_parse_multi_ic() {
    let path = fixture_path("multi_ic.kicad_pcb");
    let result = parse_kicad_pcb(&path).expect("Failed to parse multi_ic.kicad_pcb");
    let m = &result.metadata;

    assert_eq!(m.version, 20240108, "Expected KiCad 8 version");
    assert_eq!(m.layer_count, 4, "Expected 4-layer board");

    // ±20% tolerance for complex
    let expected_comps = 52usize;
    let expected_nets = 94usize;
    assert_within_tolerance(m.component_count, expected_comps, 0.20, "component_count");
    assert_within_tolerance(m.net_count, expected_nets, 0.20, "net_count");

    // Board size: 100x80mm
    assert!(m.board_size_mm.0 > 0.0, "Board width must be non-zero");
    assert!(m.board_size_mm.1 > 0.0, "Board height must be non-zero");
    assert!(
        (m.board_size_mm.0 - 100.0).abs() < 5.0,
        "Board width ~100mm, got {}",
        m.board_size_mm.0
    );
    assert!(
        (m.board_size_mm.1 - 80.0).abs() < 5.0,
        "Board height ~80mm, got {}",
        m.board_size_mm.1
    );

    // Library and world
    assert!(!result.library.is_empty(), "Library must have footprints");
    assert!(result.world.net_count() > 0, "World must have nets");

    // Reference routes present
    assert!(
        result.reference_routes.is_some(),
        "Expected reference routes"
    );
    let routes = result.reference_routes.as_ref().unwrap();
    assert!(!routes.routes.is_empty(), "Expected trace segments");
    assert!(!routes.vias.is_empty(), "Expected vias");
}

// ---------------------------------------------------------------------------
// Parametric test over all benchmarks
// ---------------------------------------------------------------------------

#[test]
fn test_all_benchmarks_parse() {
    let benchmarks = get_benchmarks();
    assert_eq!(
        benchmarks.len(),
        6,
        "Expected 6 benchmark fixtures, got {}",
        benchmarks.len()
    );

    for (bench, rel_path) in &benchmarks {
        let path = workspace_root().join(rel_path);
        assert!(
            path.exists(),
            "Benchmark fixture not found: {}",
            path.display()
        );

        let result = parse_kicad_pcb(&path)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", bench.filename, e));
        let m = &result.metadata;

        // Version is KiCad 8
        assert_eq!(
            m.version, 20240108,
            "{}: Expected KiCad 8 version",
            bench.filename
        );

        // Component count within tolerance
        let tolerance = match bench.complexity {
            BenchmarkComplexity::Simple => 0.0, // Exact for simple
            BenchmarkComplexity::Medium => 0.20,
            BenchmarkComplexity::Complex => 0.20,
        };
        assert_within_tolerance_named(
            m.component_count,
            bench.expected_component_count,
            tolerance,
            &format!("{} component_count", bench.filename),
        );

        // Net count within tolerance
        assert_within_tolerance_named(
            m.net_count,
            bench.expected_net_count,
            tolerance,
            &format!("{} net_count", bench.filename),
        );

        // Board size non-zero
        assert!(
            m.board_size_mm.0 > 0.0 && m.board_size_mm.1 > 0.0,
            "{}: Board size must be non-zero, got {:?}",
            bench.filename,
            m.board_size_mm
        );

        // Library has entries (parsed footprints registered)
        assert!(
            !result.library.is_empty(),
            "{}: Library must have footprints",
            bench.filename
        );

        // World has nets
        assert!(
            result.world.net_count() > 0,
            "{}: World must have nets",
            bench.filename
        );
    }
}

#[test]
fn test_benchmarks_constant_matches_files() {
    // Verify BENCHMARKS constant lists exactly the files that exist
    assert_eq!(BENCHMARKS.len(), 6, "Expected 6 benchmark descriptors");

    let benchmark_dir = workspace_root().join("tests/fixtures/benchmark");
    for bench in BENCHMARKS {
        let path = benchmark_dir.join(bench.filename);
        assert!(
            path.exists(),
            "Benchmark file listed in BENCHMARKS not found: {}",
            bench.filename
        );
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn assert_within_tolerance(actual: usize, expected: usize, tolerance: f64, label: &str) {
    assert_within_tolerance_named(actual, expected, tolerance, label);
}

fn assert_within_tolerance_named(actual: usize, expected: usize, tolerance: f64, label: &str) {
    if tolerance == 0.0 {
        assert_eq!(actual, expected, "{}: expected exactly {}", label, expected);
    } else {
        let lo = (expected as f64 * (1.0 - tolerance)).floor() as usize;
        let hi = (expected as f64 * (1.0 + tolerance)).ceil() as usize;
        assert!(
            actual >= lo && actual <= hi,
            "{}: expected {} ±{:.0}% (range {}..={}), got {}",
            label,
            expected,
            tolerance * 100.0,
            lo,
            hi,
            actual
        );
    }
}
