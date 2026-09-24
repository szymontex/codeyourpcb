//! Does reading a routes file leave the vias the board itself declares?
//!
//! `cargo test -p cypcb-render --test a_routes_file_leaves_the_boards_own_vias`
//!
//! The engine clears the router's copper before it reads a routes file, and
//! before each of its own routing runs. That clear asked `!via.locked` and
//! nothing about who put the via there, so the vias a board declares went with
//! the router's: `blind-via` loaded with 2 and had none after the first file.
//! It now asks for `RouterPlaced`, as `apply_routes` does, and a via read out
//! of a routes file carries that mark - it is router output, and the next file
//! has to be able to take it back.

// `load_routes` exists only in the native build; see
// `a_routes_file_carries_the_layer_across_the_crates.rs`.
#![cfg(feature = "native")]

use std::path::{Path, PathBuf};

use cypcb_render::PcbEngine;

fn example(name: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// X coordinate of every via on the board, in nanometres.
fn via_xs(engine: &mut PcbEngine) -> Vec<i64> {
    let mut xs: Vec<i64> = engine
        .build_snapshot()
        .vias
        .iter()
        .map(|via| via.x.round() as i64)
        .collect();
    xs.sort_unstable();
    xs
}

#[test]
fn the_boards_vias_outlive_two_routes_files_and_the_files_via_does_not() {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(&example("blind-via.cypcb"));
    assert!(
        errors == "[]" || errors.is_empty(),
        "the example did not load: {errors}"
    );
    let declared = via_xs(&mut engine);
    assert_eq!(
        declared,
        vec![10_000_000, 20_000_000],
        "blind-via declares two vias at 10mm and 20mm"
    );

    // A file with one via of its own, far from the board's two.
    let errors = engine.load_routes("via 0 15000000 5000000 300000 TopCopper BottomCopper\n");
    assert!(errors.is_empty(), "the routes file did not load: {errors}");
    assert_eq!(
        via_xs(&mut engine),
        vec![10_000_000, 15_000_000, 20_000_000],
        "reading a routes file deleted a via the board declares"
    );

    // An empty file replaces the router's copper with nothing.
    let errors = engine.load_routes("");
    assert!(
        errors.is_empty(),
        "the empty routes file did not load: {errors}"
    );
    assert_eq!(
        via_xs(&mut engine),
        declared,
        "the next routes file has to take back the via the last one put down, \
         and leave the board's own"
    );
}
