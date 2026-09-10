//! A four-layer board says it has four, all the way to the browser.
//!
//! `cargo test -p cypcb-render --test a_four_layer_board_says_it_has_four`
//!
//! The docked layer panel builds one row per copper layer from the board
//! itself - `copperLayerNames(boardLayerCount())`, and `boardLayerCount()` is
//! `snapshot.board.layer_count`. `examples/four-layer.cypcb` declares `layers
//! 4` and routes on `In1.Cu` and `In2.Cu`, and the panel offered two rows, so
//! either the count never left the engine or the viewer dropped it.
//!
//! This asks the engine, on the same path the browser takes: source in,
//! snapshot out. It is the half that can be answered without a browser, and it
//! is where the answer has to be right first.

use cypcb_render::PcbEngine;
use std::path::{Path, PathBuf};

fn example(name: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn the_snapshot_carries_the_layer_count_the_board_declares() {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(&example("four-layer.cypcb"));
    assert!(
        errors == "[]" || errors.is_empty(),
        "the example did not load: {errors}"
    );

    let snapshot = engine.build_snapshot();
    let board = snapshot.board.expect("a board the example declares");

    assert_eq!(
        board.layer_count, 4,
        "examples/four-layer.cypcb declares `layers 4` and the snapshot says {}",
        board.layer_count
    );
}

#[test]
fn the_inner_copper_is_in_the_snapshot_too() {
    // A count with nothing on those layers would be a number the panel could
    // show and the canvas could not draw, which is the shape of defect this
    // project has recorded three times: a row for something nothing draws.
    let mut engine = PcbEngine::new();
    engine.load_source(&example("four-layer.cypcb"));
    let snapshot = engine.build_snapshot();

    let inner: Vec<&str> = snapshot
        .traces
        .iter()
        .map(|trace| trace.layer.as_str())
        .filter(|layer| layer.starts_with("Inner"))
        .collect();

    assert!(
        inner.len() >= 2,
        "the example routes on In1.Cu and In2.Cu and the snapshot has {} inner traces: {inner:?}",
        inner.len()
    );
}
