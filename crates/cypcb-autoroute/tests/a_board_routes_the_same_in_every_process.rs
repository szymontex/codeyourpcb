//! Does a board route the same way in every run of the program?
//!
//! `the_same_board_routed_twice_is_the_same_board` routes a board three times
//! in one process, and one process cannot see this. bevy matches a query to
//! its archetypes through a hash map whose seed is drawn once per process, so
//! inside one process the order is the same every time, and across processes
//! it is not. A board whose parts do not all carry the same components sat in
//! more than one archetype, its parts came back in a different order, and
//! `examples/v2-constraints.cypcb` routed to two different boards across ten
//! runs of the same binary.
//!
//! So this runs the router in separate processes: the test starts its own
//! binary again, once per run, and each child prints what it routed. The seed
//! cannot be set from outside, which is why the count is high enough that a
//! board with two outcomes shows both.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world};
use cypcb_rules::presets::RulesPreset;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// The examples that routed to more than one board across processes. Each has
/// parts that do not all carry the same components.
const BOARDS: &[&str] = &["v2-constraints", "v2-interfaces", "v2-modules"];

/// Runs of the program. The rarest of the two outcomes came up two times in
/// ten, so sixteen runs all landing on the same one by chance is under one in
/// thirty.
const PROCESSES: usize = 16;

/// Set in a child, which routes and prints instead of starting children.
const CHILD: &str = "CYPCB_ROUTE_IN_CHILD";

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(format!("{name}.cypcb"))
}

/// One board routed, as a hash of every segment and via it laid.
fn route(name: &str) -> u64 {
    let source = std::fs::read_to_string(example(name))
        .unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
    let parsed = cypcb_parser::parse(&source);
    assert!(parsed.is_ok(), "{name} has to parse");
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let _ = sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);

    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let rules = ruleset_for_world(preset, &world);
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    assert!(!result.routes.is_empty(), "{name} has to route something");

    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for s in &result.routes {
        (
            s.net_id.0,
            s.layer,
            s.start.x.0,
            s.start.y.0,
            s.end.x.0,
            s.end.y.0,
        )
            .hash(&mut hash);
    }
    for v in &result.vias {
        (
            v.net_id.0,
            v.start_layer,
            v.end_layer,
            v.position.x.0,
            v.position.y.0,
        )
            .hash(&mut hash);
    }
    hash.finish()
}

#[test]
fn a_board_routes_the_same_in_every_process() {
    if std::env::var_os(CHILD).is_some() {
        for name in BOARDS {
            println!("ROUTED {name} {:016x}", route(name));
        }
        return;
    }

    let exe = std::env::current_exe().expect("the test binary has a path");
    let mut seen: Vec<Vec<String>> = vec![Vec::new(); BOARDS.len()];
    for _ in 0..PROCESSES {
        let output = Command::new(&exe)
            .args([
                "--exact",
                "a_board_routes_the_same_in_every_process",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .expect("the test binary starts again");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success(), "a child failed:\n{stdout}");

        for (index, name) in BOARDS.iter().enumerate() {
            let prefix = format!("ROUTED {name} ");
            let hash = stdout
                .lines()
                .find_map(|line| line.split_once(&prefix).map(|(_, h)| h.to_string()))
                .unwrap_or_else(|| panic!("a child routed nothing for {name}:\n{stdout}"));
            if !seen[index].contains(&hash) {
                seen[index].push(hash);
            }
        }
    }

    let unstable: Vec<String> = BOARDS
        .iter()
        .zip(&seen)
        .filter(|(_, hashes)| hashes.len() > 1)
        .map(|(name, hashes)| format!("{name} ({})", hashes.join(", ")))
        .collect();
    assert!(
        unstable.is_empty(),
        "the same board routed {PROCESSES} times in separate processes has to be the \
         same board; these were not: {}",
        unstable.join("; ")
    );
}
