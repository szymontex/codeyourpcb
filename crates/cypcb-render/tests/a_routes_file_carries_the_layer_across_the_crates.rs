//! A routes file carries the layer from the CLI's pen to the viewer's eye.
//!
//! `cypcb route` writes `segment {net} {layer:?} {width} {x1} {y1} {x2} {y2}`
//! and the engine reads that file back with `load_routes`. Two crates, one
//! format, and the layer passes through the debug spelling on the way: the
//! enum counts inner layers from zero, so `Inner(1)` in the file is the second
//! inner layer, which the viewer shows as `Inner2`.
//!
//! Each half had a case of its own and nothing carried a file across.
//!
//! **The copper is found by its own coordinates, not by its layer.** The
//! four-layer example already draws on both inner layers, so asking whether
//! the snapshot holds an `Inner2` trace would answer yes with the file never
//! read - the question has to be which layer *this* segment landed on.
//!
//! `cargo test -p cypcb-render --test a_routes_file_carries_the_layer_across_the_crates`

use std::path::{Path, PathBuf};

use cypcb_render::PcbEngine;

/// The end of each segment this file writes, far from anything the example
/// draws, so a trace can be found by where it ends.
const ENDS: [(i64, &str, &str); 3] = [
    (2_000_000, "TopCopper", "Top"),
    (3_000_000, "Inner(0)", "Inner1"),
    (4_000_000, "Inner(1)", "Inner2"),
];

fn example(name: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The layer of every trace with a segment ending at `end_x`.
fn layers_ending_at(engine: &mut PcbEngine, end_x: i64) -> Vec<String> {
    engine
        .build_snapshot()
        .traces
        .iter()
        .filter(|trace| {
            trace
                .segments
                .iter()
                .any(|segment| (segment.end_x - end_x as f64).abs() < 1.0)
        })
        .map(|trace| trace.layer.clone())
        .collect()
}

#[test]
fn the_layer_a_routes_file_names_is_the_layer_the_snapshot_shows() {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(&example("four-layer.cypcb"));
    assert!(
        errors == "[]" || errors.is_empty(),
        "the example did not load: {errors}"
    );

    // The control: nothing ends where this file's copper will end, so a trace
    // found there afterwards came from the file and from nowhere else.
    for (end_x, _, _) in ENDS {
        let before = layers_ending_at(&mut engine, end_x);
        assert!(
            before.is_empty(),
            "the example already draws copper ending at {end_x}: {before:?}"
        );
    }

    let routes: String = ENDS
        .iter()
        .map(|(end_x, written, _)| {
            format!("segment 1 {written} 200000 1000000 1000000 {end_x} 1000000\n")
        })
        .collect();
    let errors = engine.load_routes(&format!("version 1\n{routes}"));
    assert!(
        errors == "[]" || errors.is_empty(),
        "the routes file the CLI writes did not load: {errors}"
    );

    for (end_x, written, shown) in ENDS {
        let found = layers_ending_at(&mut engine, end_x);
        assert_eq!(
            found,
            vec![shown.to_string()],
            "the file said {written} and the snapshot shows {found:?}"
        );
    }
}
