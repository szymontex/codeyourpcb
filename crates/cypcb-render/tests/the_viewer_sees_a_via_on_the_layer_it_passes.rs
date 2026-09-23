//! The viewer's checker sees a via on the layers it passes, as the CLI's does.
//!
//! `cargo test -p cypcb-render --test the_viewer_sees_a_via_on_the_layer_it_passes`
//!
//! The engine keeps a spatial index of its own, built beside the one in
//! `cypcb-world`, and until 2026-09-24 both gave a via only the two layers it
//! joins. A Top-to-Inner2 via crossed on Inner1 by another net's track was
//! quiet in the browser; this loads that crossing from a routes file and asks
//! the engine's own DRC.

#![cfg(feature = "native")]

use cypcb_render::PcbEngine;

const BOARD: &str = "board t {\n    size 20mm x 20mm\n    layers 4\n}\n";

/// Clearance reports with a Top-to-Inner2 via of net 1 at 10mm, 10mm and a
/// track of net 2 across it on `track_layer`.
fn clearance_reports(track_layer: &str) -> usize {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(BOARD);
    assert!(
        errors == "[]" || errors.is_empty(),
        "the board did not load: {errors}"
    );
    let errors = engine.load_routes(&format!(
        "version 1\n\
         via 1 10000000 10000000 300000 TopCopper Inner(1)\n\
         segment 2 {track_layer} 200000 5000000 10000000 15000000 10000000\n"
    ));
    assert!(errors.is_empty(), "the routes did not load: {errors}");
    engine.run_drc_incremental();
    let violations: Vec<serde_json::Value> =
        serde_json::from_str(&engine.get_violations_json()).expect("the engine writes JSON");
    violations
        .iter()
        .filter(|violation| violation["kind"] == "clearance")
        .count()
}

#[test]
fn a_track_across_a_blind_via_on_the_layer_it_passes_is_reported() {
    assert_eq!(clearance_reports("Inner(0)"), 1, "the layer it passes");
    // The controls: the layer it joins is reported, the face it stops short
    // of is not.
    assert_eq!(clearance_reports("Inner(1)"), 1, "the layer it joins");
    assert_eq!(
        clearance_reports("BottomCopper"),
        0,
        "the face it stops short of"
    );
}
