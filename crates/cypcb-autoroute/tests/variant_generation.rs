//! Integration test for multi-variant generation.
//!
//! Parses the led_blink KiCad fixture, generates variants with default configs,
//! and verifies ranking, scoring, and world state.

use std::path::Path;

use cypcb_autoroute::variant::{default_variant_configs, generate_variants};
use cypcb_drc::DesignRules;
use cypcb_kicad::parse_kicad_pcb;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::trace::Trace;

// ============================================================================
// Helpers
// ============================================================================

/// Resolve a benchmark fixture path relative to workspace root.
fn fixture_path(filename: &str) -> std::path::PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// Build JLCPCB 2-layer routing rules.
fn test_rules() -> PresetRuleSet {
    let preset = RulesPreset::from_name("jlcpcb").unwrap();
    PresetRuleSet::new(preset)
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn variant_generation_returns_multiple_results() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("cypcb_autoroute::variant=info")
        .with_test_writer()
        .try_init();

    let parsed = parse_kicad_pcb(&fixture_path("led_blink.kicad_pcb"))
        .expect("Failed to parse led_blink fixture");
    let mut world = parsed.world;
    let library = parsed.library;
    let rules = test_rules();
    let design_rules = DesignRules::default();
    let configs = default_variant_configs();

    let results = generate_variants(&mut world, &library, &rules, &design_rules, &configs);

    // Should have at least 3 successful variants (some may fail, but most should succeed)
    assert!(
        results.len() >= 3,
        "Expected at least 3 variants, got {}",
        results.len()
    );
}

#[test]
fn variants_sorted_by_composite_score() {
    let parsed = parse_kicad_pcb(&fixture_path("led_blink.kicad_pcb"))
        .expect("Failed to parse led_blink fixture");
    let mut world = parsed.world;
    let library = parsed.library;
    let rules = test_rules();
    let design_rules = DesignRules::default();
    let configs = default_variant_configs();

    let results = generate_variants(&mut world, &library, &rules, &design_rules, &configs);

    assert!(results.len() >= 2, "Need at least 2 variants to test sorting");

    // Verify ascending composite score order
    for window in results.windows(2) {
        assert!(
            window[0].score.composite <= window[1].score.composite,
            "Variants not sorted: {} ({}) should be <= {} ({})",
            window[0].name,
            window[0].score.composite,
            window[1].name,
            window[1].score.composite
        );
    }

    // Best variant is first (lowest composite)
    let best = &results[0];
    for r in &results[1..] {
        assert!(
            best.score.composite <= r.score.composite,
            "Best variant '{}' ({}) should have lowest composite, but '{}' has {}",
            best.name,
            best.score.composite,
            r.name,
            r.score.composite
        );
    }
}

#[test]
fn all_variants_have_routes() {
    let parsed = parse_kicad_pcb(&fixture_path("led_blink.kicad_pcb"))
        .expect("Failed to parse led_blink fixture");
    let mut world = parsed.world;
    let library = parsed.library;
    let rules = test_rules();
    let design_rules = DesignRules::default();
    let configs = default_variant_configs();

    let results = generate_variants(&mut world, &library, &rules, &design_rules, &configs);

    for result in &results {
        assert!(
            !result.routes.is_empty(),
            "Variant '{}' has no routes",
            result.name
        );
    }
}

#[test]
fn best_variant_applied_to_world() {
    let parsed = parse_kicad_pcb(&fixture_path("led_blink.kicad_pcb"))
        .expect("Failed to parse led_blink fixture");
    let mut world = parsed.world;
    let library = parsed.library;
    let rules = test_rules();
    let design_rules = DesignRules::default();
    let configs = default_variant_configs();

    let results = generate_variants(&mut world, &library, &rules, &design_rules, &configs);

    assert!(!results.is_empty(), "No variants generated");

    // After generation, world should have traces (from the best variant)
    let trace_count = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).count()
    };

    assert!(
        trace_count > 0,
        "Expected traces in world after variant generation, got 0"
    );
}

#[test]
fn variant_results_serialize_to_json() {
    let parsed = parse_kicad_pcb(&fixture_path("led_blink.kicad_pcb"))
        .expect("Failed to parse led_blink fixture");
    let mut world = parsed.world;
    let library = parsed.library;
    let rules = test_rules();
    let design_rules = DesignRules::default();
    let configs = default_variant_configs();

    let results = generate_variants(&mut world, &library, &rules, &design_rules, &configs);

    // Serialize to JSON (this is what the WASM bridge does)
    let json = serde_json::to_string(&results).expect("Results should serialize to JSON");

    // Basic structure validation
    assert!(json.starts_with('['), "Should be a JSON array");

    // Parse back and validate structure
    let parsed_json: serde_json::Value =
        serde_json::from_str(&json).expect("Should be valid JSON");
    let arr = parsed_json.as_array().expect("Should be an array");
    assert!(arr.len() >= 3);

    // Each element should have the expected fields
    for item in arr {
        assert!(item.get("name").is_some(), "Missing 'name' field");
        assert!(item.get("score").is_some(), "Missing 'score' field");
        assert!(item.get("routes").is_some(), "Missing 'routes' field");
        assert!(item.get("vias").is_some(), "Missing 'vias' field");

        let score = item.get("score").unwrap();
        assert!(
            score.get("composite").is_some(),
            "Missing 'composite' in score"
        );
    }
}
