//! What the teardrop copper costs each time the board is rebuilt.
//!
//! `cargo test --release -p cypcb-autoroute --test what_a_rebuild_costs_with_teardrops -- --ignored --nocapture`
//!
//! The copper is rebuilt by `rebuild_spatial_index_from_library`, which every
//! caller runs after it changes a track. This measures that call on
//! `multi_ic`, the largest benchmark, as the board is without a request for
//! teardrops and with one. It prints; the first line says the host's load.

use std::path::Path;
use std::time::Instant;

use cypcb_kicad::parse_kicad_pcb;
use cypcb_world::components::Teardrops;
use cypcb_world::teardrop::teardrop_copper;

fn median(mut times: Vec<f64>) -> f64 {
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times[times.len() / 2]
}

#[test]
#[ignore = "measures; run on purpose"]
fn the_rebuild_on_multi_ic() {
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    println!("load average at the start: {}", load.trim());

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark/multi_ic.kicad_pcb");
    let parsed = parse_kicad_pcb(&path).expect("multi_ic reads");
    let mut world = parsed.world;
    let library = parsed.library;
    world.set_footprints(library.clone());
    // The board as it is when there is copper to rebuild around: routed.
    let rules = cypcb_rules::presets::PresetRuleSet::new(
        cypcb_rules::presets::RulesPreset::from_name("jlcpcb").expect("the preset"),
    );
    let result = cypcb_autoroute::route_board(
        &mut world,
        &library,
        &rules,
        &cypcb_autoroute::AutorouteConfig::default(),
    );
    cypcb_router::apply_routes(&mut world, &result);
    let tracks =
        cypcb_world::in_build_order::<&cypcb_world::components::trace::Trace>(world.ecs_mut())
            .len();
    println!("tracks on the routed board: {tracks}");

    for asked in [false, true] {
        if asked {
            assert!(world.set_teardrops(Teardrops::default()));
        }
        let mut times = Vec::new();
        for _ in 0..21 {
            let start = Instant::now();
            world.rebuild_spatial_index_from_library(&library);
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let held = teardrop_copper(&mut world).len();
        // The first of the 21 builds the copper, the rest rebuild it.
        println!(
            "teardrops asked: {asked:5} | rebuild median {:.3} ms, max {:.3} ms | {held} teardrops held",
            median(times.clone()),
            times.iter().cloned().fold(0.0, f64::max)
        );
    }
}
