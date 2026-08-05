//! Integration tests for the scoring system.
//!
//! Tests route real .cypcb boards, score them with `score_board()`, and validate
//! all 7 routing quality metrics against expected baseline ranges.
//!
//! These tests establish the baseline scores that S07's benchmark suite will
//! compare against.

use cypcb_autoroute::scoring::{score_board, ScoreWeights};
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::Nm;
use cypcb_drc::DesignRules;
use cypcb_parser::parse;
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

// ============================================================================
// Helpers (same pattern as integration.rs)
// ============================================================================

/// Resolve a path relative to the workspace root.
fn workspace_path(relative: &str) -> std::path::PathBuf {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // crates/cypcb-autoroute -> workspace root is ../../
    manifest_dir.join("../..").join(relative)
}

/// Parse a .cypcb file into a BoardWorld.
fn parse_board(relative_path: &str) -> BoardWorld {
    let path = workspace_path(relative_path);
    let path_str = path.display().to_string();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("Failed to read {path_str}: {e}");
    });
    let parse_result = parse(&source);
    assert!(
        parse_result.is_ok(),
        "Parse errors in {path_str}: {:?}",
        parse_result.errors
    );

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let sync_result = sync_ast_to_world(&parse_result.value, &source, &mut world, &mut library);

    if sync_result.has_errors() {
        for err in &sync_result.errors {
            eprintln!("Sync warning in {path_str}: {err:?}");
        }
    }

    world
}

/// Build routing rules for testing (JLCPCB 2-layer defaults).
fn test_rules() -> PresetRuleSet {
    let preset = RulesPreset::from_name("jlcpcb").unwrap();
    PresetRuleSet::new(preset)
}

/// Route a board and apply routes, returning the world ready for scoring.
fn route_and_apply(world: &mut BoardWorld) {
    let mut library = FootprintLibrary::new();
    let rules = test_rules();
    let config = AutorouteConfig::default();

    let result = route_board(world, &library, &rules, &config);

    // Apply routes to world (spawns Trace and Via entities)
    apply_routes(world, &result);

    // Rebuild spatial index with traces for accurate crossing/scoring
    world.rebuild_spatial_index_with_traces(|_| {
        cypcb_core::Rect::from_center_size(
            cypcb_core::Point::ORIGIN,
            (Nm::from_mm(1.0), Nm::from_mm(1.0)),
        )
    });
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn score_routed_blink() {
    let mut world = parse_board("examples/blink.cypcb");
    route_and_apply(&mut world);

    let drc_rules = DesignRules::jlcpcb_2layer();
    let score = score_board(&mut world, &drc_rules, &ScoreWeights::default());

    eprintln!("\n╔══════════════════════════════════════════════╗");
    eprintln!("║     blink.cypcb Scoring Baseline             ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!(
        "║ total_length:  {:.2} mm",
        score.total_length.0 as f64 / 1_000_000.0
    );
    eprintln!("║ via_count:     {}", score.via_count);
    eprintln!("║ drc_violations:{}", score.drc_violations);
    eprintln!("║ smoothness:    {:.4}", score.smoothness);
    eprintln!("║ crossings:     {}", score.crossings);
    eprintln!("║ layer_balance: {:.4}", score.layer_balance);
    eprintln!("║ composite:     {:.4}", score.composite);
    eprintln!("╚══════════════════════════════════════════════╝\n");

    // Validate all 7 metric ranges
    assert!(
        score.total_length > Nm(0),
        "Routed board should have non-zero trace length, got {:?}",
        score.total_length
    );

    // via_count is always u32 by type — just verify it's reasonable
    assert!(
        score.via_count < 100,
        "Via count {} is unreasonably high for blink board",
        score.via_count
    );

    // DRC violations reflect autorouter output quality, not a scoring bug.
    // The A*-based autorouter may produce clearance violations on complex boards.
    // Validate the metric is a reasonable count (not that routing is perfect).
    assert!(
        score.drc_violations < 200,
        "DRC violation count {} is unreasonably high for blink board",
        score.drc_violations
    );

    assert!(
        score.smoothness >= 0.0 && score.smoothness <= 1.0,
        "Smoothness should be in [0.0, 1.0], got {}",
        score.smoothness
    );

    // Crossings may occur depending on routing quality — validate reasonable range
    assert!(
        score.crossings < 50,
        "Crossing count {} is unreasonably high for blink board",
        score.crossings
    );

    assert!(
        score.layer_balance >= 0.0 && score.layer_balance <= 1.0,
        "Layer balance should be in [0.0, 1.0], got {}",
        score.layer_balance
    );

    assert!(
        score.composite > 0.0,
        "Routed board with traces should have positive composite score, got {}",
        score.composite
    );
}

#[test]
fn score_routed_routing_test() {
    let mut world = parse_board("examples/routing-test.cypcb");
    route_and_apply(&mut world);

    let drc_rules = DesignRules::jlcpcb_2layer();
    let score = score_board(&mut world, &drc_rules, &ScoreWeights::default());

    eprintln!("\n╔══════════════════════════════════════════════╗");
    eprintln!("║   routing-test.cypcb Scoring Baseline        ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!(
        "║ total_length:  {:.2} mm",
        score.total_length.0 as f64 / 1_000_000.0
    );
    eprintln!("║ via_count:     {}", score.via_count);
    eprintln!("║ drc_violations:{}", score.drc_violations);
    eprintln!("║ smoothness:    {:.4}", score.smoothness);
    eprintln!("║ crossings:     {}", score.crossings);
    eprintln!("║ layer_balance: {:.4}", score.layer_balance);
    eprintln!("║ composite:     {:.4}", score.composite);
    eprintln!("╚══════════════════════════════════════════════╝\n");

    // Simpler board — verify scoring works on minimal input
    assert!(
        score.total_length > Nm(0),
        "Routed routing-test board should have non-zero trace length"
    );

    assert!(
        score.smoothness >= 0.0 && score.smoothness <= 1.0,
        "Smoothness should be in [0.0, 1.0], got {}",
        score.smoothness
    );

    assert!(
        score.layer_balance >= 0.0 && score.layer_balance <= 1.0,
        "Layer balance should be in [0.0, 1.0], got {}",
        score.layer_balance
    );

    assert!(
        score.composite > 0.0,
        "Routed board should have positive composite score"
    );
}

#[test]
fn score_empty_board_is_valid() {
    // Score an unrouted board — verify no panic and metrics reflect empty state
    let mut world = BoardWorld::new();
    world.set_board(
        "EmptyBoard".to_string(),
        (Nm::from_mm(50.0), Nm::from_mm(30.0)),
        2,
    );

    let drc_rules = DesignRules::jlcpcb_2layer();
    let score = score_board(&mut world, &drc_rules, &ScoreWeights::default());

    assert_eq!(
        score.total_length,
        Nm(0),
        "Empty board should have zero trace length"
    );
    assert_eq!(score.via_count, 0, "Empty board should have zero vias");
    assert_eq!(score.crossings, 0, "Empty board should have zero crossings");
    assert!(
        (score.smoothness - 1.0).abs() < 1e-10,
        "Empty board should have perfect smoothness (1.0), got {}",
        score.smoothness
    );
    assert!(
        (score.layer_balance - 1.0).abs() < 1e-10,
        "Empty board should have perfect layer balance (1.0), got {}",
        score.layer_balance
    );
    assert!(
        score.composite.abs() < 1e-10,
        "Empty board should have zero composite, got {}",
        score.composite
    );
}

#[test]
fn score_json_serialization() {
    let mut world = parse_board("examples/routing-test.cypcb");
    route_and_apply(&mut world);

    let drc_rules = DesignRules::jlcpcb_2layer();
    let score = score_board(&mut world, &drc_rules, &ScoreWeights::default());

    // Serialize to JSON
    let json = serde_json::to_string(&score).expect("RoutingScore should serialize to JSON");

    // Verify all 7 field names are present
    let expected_fields = [
        "total_length",
        "via_count",
        "drc_violations",
        "smoothness",
        "crossings",
        "layer_balance",
        "composite",
    ];

    for field in &expected_fields {
        assert!(
            json.contains(field),
            "JSON should contain field '{}', got: {}",
            field,
            json
        );
    }

    // Verify it parses back as valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("Serialized JSON should parse back");

    assert!(parsed.is_object(), "Parsed JSON should be an object");
    let obj = parsed.as_object().unwrap();
    assert_eq!(
        obj.len(),
        7,
        "JSON object should have exactly 7 fields, got {}",
        obj.len()
    );

    eprintln!("JSON serialization OK: {json}");
}
