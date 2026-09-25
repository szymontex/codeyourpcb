//! Does the ranking take a via kept off traces where it helps, and only there?
//!
//! Two variants charge a via 1000 for each cell of another net's trace its
//! copper would touch: one on top of High-Density, one on top of Default. Each
//! routes one board with fewer shorts than the variant it is built on and
//! another board worse. A variant only earns its place if the ranking picks it
//! on the first board and not on the second, so each is run side by side with
//! its base, both taken from the shipped list by name.

use std::path::Path;

use cypcb_autoroute::variant::{default_variant_configs, generate_variants, VariantConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, DesignRules};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_rules::presets::RulesPreset;

const HIGH_DENSITY: &str = "PathFinder High-Density";
const HIGH_DENSITY_KEPT_OFF: &str = "PathFinder High-Density Vias Kept Off Traces";
const DEFAULT: &str = "PathFinder Default";
const DEFAULT_KEPT_OFF: &str = "PathFinder Vias Kept Off Traces";

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// The variants as the shipped list carries them. A variant taken out of the
/// list fails here, not somewhere downstream.
fn shipped(names: [&str; 2]) -> Vec<VariantConfig> {
    let list = default_variant_configs();
    names
        .iter()
        .map(|name| {
            list.iter()
                .find(|c| c.name == *name)
                .unwrap_or_else(|| panic!("`{name}` is not in default_variant_configs"))
                .clone()
        })
        .collect()
}

fn winner_on(filename: &str, names: [&str; 2]) -> String {
    let parsed = parse_kicad_pcb(&fixture_path(filename))
        .unwrap_or_else(|e| panic!("Failed to parse {filename}: {e:?}"));
    let mut world = parsed.world;
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let rules = ruleset_for_world(preset, &world);
    let design_rules = DesignRules::from_constraints(&preset.constraints());
    let results = generate_variants(
        &mut world,
        &parsed.library,
        &rules,
        &design_rules,
        &shipped(names),
    );
    for r in &results {
        eprintln!(
            "{filename}: {} - {} shorts, {} incomplete",
            r.name,
            r.score.shorts,
            r.incomplete()
        );
    }
    results.first().expect("a variant routed").name.clone()
}

#[test]
fn high_density_kept_off_traces_wins_stm32_breakout() {
    let pair = [HIGH_DENSITY, HIGH_DENSITY_KEPT_OFF];
    assert_eq!(
        winner_on("stm32_breakout.kicad_pcb", pair),
        HIGH_DENSITY_KEPT_OFF
    );
}

#[test]
fn high_density_kept_off_traces_loses_shift_driver() {
    let pair = [HIGH_DENSITY, HIGH_DENSITY_KEPT_OFF];
    assert_eq!(winner_on("shift_driver.kicad_pcb", pair), HIGH_DENSITY);
}

#[test]
fn default_kept_off_traces_wins_qfp_fanout() {
    let pair = [DEFAULT, DEFAULT_KEPT_OFF];
    assert_eq!(winner_on("qfp_fanout.kicad_pcb", pair), DEFAULT_KEPT_OFF);
}

#[test]
fn default_kept_off_traces_loses_stm32_breakout() {
    let pair = [DEFAULT, DEFAULT_KEPT_OFF];
    assert_eq!(winner_on("stm32_breakout.kicad_pcb", pair), DEFAULT);
}
