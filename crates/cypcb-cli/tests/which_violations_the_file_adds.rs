//! Diagnostic: which violations appear only after the board is written out.
//!
//! `cargo test -p cypcb-cli --test which_violations_the_file_adds -- --ignored --nocapture`
//!
//! `route` measures the world it routed and `check` measures the file it
//! wrote, and on `examples/blink.cypcb` they answer 4 and 5. This prints both
//! lists side by side so the difference is a name and a coordinate rather than
//! a count.

use cypcb_drc::{run_drc, DesignRules};
use cypcb_rules::presets::RulesPreset;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn world_from(source: &str) -> (BoardWorld, FootprintLibrary) {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    (world, library)
}

fn listed(world: &mut BoardWorld, library: &FootprintLibrary) -> Vec<String> {
    world.rebuild_spatial_index_from_library(library);
    let rules = DesignRules::from_constraints(&RulesPreset::JlcpcbStandard2Layer.constraints());
    let mut lines: Vec<String> = run_drc(world, &rules)
        .violations
        .iter()
        .map(|violation| {
            format!(
                "{:?} at ({:.3}, {:.3}) actual {:?}",
                violation.kind,
                violation.location.x.0 as f64 / 1e6,
                violation.location.y.0 as f64 / 1e6,
                violation.actual.map(|nm| nm.0)
            )
        })
        .collect();
    lines.sort();
    lines
}

#[test]
#[ignore = "diagnostic: prints the violations a write-and-read-back adds"]
fn the_two_lists_side_by_side() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("the crate sits two levels below the repo root");
    let source =
        std::fs::read_to_string(root.join("examples/blink.cypcb")).expect("the example is there");

    // 1. Route it, the way `cypcb route --in-house` does.
    let (mut world, library) = world_from(&source);
    let rule_set = cypcb_rules::presets::PresetRuleSet::new(RulesPreset::JlcpcbStandard2Layer);
    // The variant path, because that is what `cypcb route --in-house` runs.
    let design_rules =
        DesignRules::from_constraints(&RulesPreset::JlcpcbStandard2Layer.constraints());
    let configs = cypcb_autoroute::variant::default_variant_configs();
    let results = cypcb_autoroute::variant::generate_variants(
        &mut world,
        &library,
        &rule_set,
        &design_rules,
        &configs,
    );
    let best = results.first().expect("a variant survived");
    println!(
        "chose {} - scored {} violations, {} shorts",
        best.name, best.score.drc_violations, best.score.shorts
    );
    let result =
        cypcb_router::types::RoutingResult::complete(best.routes.clone(), best.vias.clone());
    cypcb_router::apply_routes(&mut world, &result);

    let in_memory = listed(&mut world, &library);

    // 2. Write it out and read it back, the way `cypcb check` sees it.
    let traces = cypcb_world::dsl::traces_as_dsl(&mut world);
    let written = format!("{source}\n{traces}");
    let (mut reread, reread_library) = world_from(&written);
    let from_file = listed(&mut reread, &reread_library);

    println!("--- in memory: {} ---", in_memory.len());
    for line in &in_memory {
        println!("  {line}");
    }
    println!("--- from the file: {} ---", from_file.len());
    for line in &from_file {
        println!("  {line}");
    }
    println!("--- only in the file ---");
    for line in &from_file {
        if !in_memory.contains(line) {
            println!("  {line}");
        }
    }
    println!("--- only in memory ---");
    for line in &in_memory {
        if !from_file.contains(line) {
            println!("  {line}");
        }
    }
}
