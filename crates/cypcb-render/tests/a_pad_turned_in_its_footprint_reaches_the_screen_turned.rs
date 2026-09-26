//! A pad turned inside its footprint reaches the browser turned.
//!
//! `cargo test -p cypcb-render --test a_pad_turned_in_its_footprint_reaches_the_screen_turned`
//!
//! The browser learns a board from two places: the snapshot, which draws it,
//! and the engine's own index, which the checker walks while the design is
//! edited. Both turned a pad with its part and knew nothing of the pad's own
//! turn, so a header whose pads stand across the part was drawn and checked
//! as if they stood along it.

use cypcb_render::PcbEngine;

/// Two parts 3.1mm apart, each one 0.4mm x 3mm pad, on different nets.
///
/// Turned a quarter inside the footprint the pads are 3mm across and leave a
/// 0.1mm gap, under the 0.127mm the board asks for; standing square they are
/// 0.4mm across and leave 2.7mm.
fn board(turn: &str) -> String {
    format!(
        r#"version 1

footprint BAR {{
    courtyard 3.2mm x 3.2mm
    pad 1 rect at 0mm, 0mm{turn} size 0.4mm x 3mm
}}

board b {{
    size 30mm x 20mm
    layers 2
}}

component U1 ic "BAR" {{
    at 10mm, 10mm
}}

component U2 ic "BAR" {{
    at 13.1mm, 10mm
}}

net A {{ U1.1 }}
net B {{ U2.1 }}
"#
    )
}

fn engine(turn: &str) -> PcbEngine {
    let mut engine = PcbEngine::new();
    let report = engine.load_source(&board(turn));
    assert!(
        !report.to_lowercase().contains("error"),
        "the board loads: {report}"
    );
    engine
}

fn clearance_violations(engine: &PcbEngine) -> usize {
    engine
        .get_violations_json()
        .matches("\"kind\":\"clearance\"")
        .count()
}

#[test]
fn the_snapshot_hands_the_viewer_the_pad_as_it_stands() {
    // The viewer turns a pad with its part only, so it is handed the pad's
    // sides along the footprint's axes.
    let turned = engine(" rotate 90").get_snapshot();
    assert!(
        turned.contains("\"width_nm\":3000000,\"height_nm\":400000"),
        "the turned pad reaches the viewer standing the other way: {turned}"
    );
    let square = engine("").get_snapshot();
    assert!(
        square.contains("\"width_nm\":400000,\"height_nm\":3000000"),
        "the square pad reaches the viewer as written: {square}"
    );
}

#[test]
fn the_checker_in_the_browser_measures_the_pad_as_it_stands() {
    let mut turned = engine(" rotate 90");
    turned.run_drc_incremental();
    let mut square = engine("");
    square.run_drc_incremental();
    assert_eq!(
        clearance_violations(&square),
        0,
        "2.7mm apart is clear: {}",
        square.get_violations_json()
    );
    assert!(
        clearance_violations(&turned) > 0,
        "0.1mm apart is not clear: {}",
        turned.get_violations_json()
    );
}
