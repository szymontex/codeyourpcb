//! The viewer and the command line read a KiCad board's copper alike.
//!
//! `cargo test -p cypcb-cli --test the_viewer_and_the_command_line_read_a_kicad_board_alike`
//!
//! Both take the file's own copper from the importer's reference routes. The
//! command line marked it drawn by hand and the viewer marked it the router's,
//! so the first autoroute in the viewer ripped up copper the person drew in
//! KiCad, and a save named it routed copper written down as drawn by hand.
//!
//! The engine is asked through what it already tells the viewer: its snapshot
//! for the copper, and `design_not_written` for how much of it is the router's.

use std::path::{Path, PathBuf};

use cypcb_render::PcbEngine;
use cypcb_world::components::trace::{Trace, TraceSource};

fn benchmark() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/benchmark");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("the benchmark")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "kicad_pcb"))
        .collect();
    files.sort();
    files
}

/// What `from-kicad` builds before it writes: the importer's world and the
/// copper the file carries, drawn by hand (`commands/from_kicad.rs`). Returns
/// how many traces it holds and how many of those are the router's.
fn command_line(path: &Path) -> (usize, usize) {
    let parsed = cypcb_kicad::parse_kicad_pcb(path).expect("the board parses");
    let mut world = parsed.world;
    if let Some(routes) = parsed.reference_routes {
        cypcb_router::apply_routes_as(&mut world, &routes, TraceSource::Manual);
    }
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Trace>();
    let sources: Vec<TraceSource> = query.iter(ecs).map(|t| t.source).collect();
    let routed = sources
        .iter()
        .filter(|s| **s == TraceSource::Autorouted)
        .count();
    (sources.len(), routed)
}

/// The same two numbers from the engine the viewer runs.
fn viewer(text: &str) -> (usize, usize) {
    let mut engine = PcbEngine::new();
    assert_eq!(engine.load_kicad(text), "");
    let snapshot: serde_json::Value =
        serde_json::from_str(&engine.get_snapshot()).expect("the snapshot is JSON");
    let traces = snapshot["traces"].as_array().map_or(0, Vec::len);
    let routed = engine
        .design_not_written()
        .lines()
        .find_map(|line| {
            line.strip_suffix(" autorouted trace(s) written as drawn by hand")
                .map(|n| n.parse::<usize>().expect("a count"))
        })
        .unwrap_or(0);
    (traces, routed)
}

#[test]
fn each_trace_is_drawn_by_the_same_hand_in_both() {
    let mut traces = 0;
    for path in benchmark() {
        let text = std::fs::read_to_string(&path).expect("reads");
        let (seen, routed) = viewer(&text);
        assert_eq!((seen, routed), command_line(&path), "{}", path.display());
        assert_eq!(
            routed,
            0,
            "{}: KiCad's copper read as the router's",
            path.display()
        );
        traces += seen;
    }
    // The control: a comparison of two boards without copper agrees about
    // nothing.
    assert!(traces > 0, "no benchmark board carries copper");
}
