//! What the browser is handed for a part on the back of the board.
//!
//! `cargo test -p cypcb-render --features native --test a_bottom_part_reaches_the_browser_flipped`
//!
//! `side bottom` was added to the language and proved through the export. The
//! viewer is the other consumer, and the question was whether it needed the
//! same work: the tracker said the browser builds its own snapshot with a
//! TypeScript reader, which stopped being true on 2026-08-07 when that reader
//! was deleted and `WasmPcbEngineAdapter.load_source` started calling the
//! engine.
//!
//! So this drives the engine exactly as the browser does and reads what comes
//! back. The renderer draws a pad from its `layer_mask`, so a part on the
//! bottom has to arrive with bottom-layer pads and mirrored geometry, or the
//! screen and the Gerbers describe different boards.

use cypcb_render::PcbEngine;

/// The same part, on whichever face the caller asks for.
fn board(side: &str) -> String {
    format!(
        r#"version 1

board two_sided {{
    size 30mm x 20mm
    layers 2
}}

component R1 resistor "0402" {{
    value "10k"
    at 10mm, 10mm
{side}
}}
"#
    )
}

fn snapshot(source: &str) -> serde_json::Value {
    let mut engine = PcbEngine::new();
    engine.load_source(source);
    serde_json::from_str(&engine.get_snapshot()).expect("the snapshot is JSON")
}

/// The pads of the first component, as (layer mask, x in nm).
fn pads(snapshot: &serde_json::Value) -> Vec<(u64, i64)> {
    snapshot["components"][0]["pads"]
        .as_array()
        .expect("the part has pads")
        .iter()
        .map(|pad| {
            (
                pad["layer_mask"].as_u64().expect("a layer mask"),
                pad["x_nm"].as_i64().expect("an x"),
            )
        })
        .collect()
}

/// Copper layer bits, as `Layer::to_copper_mask` defines them.
const TOP: u64 = 0b01;
const BOTTOM: u64 = 0b10;

#[test]
fn its_pads_arrive_on_the_bottom_copper() {
    let flipped = pads(&snapshot(&board("    side bottom")));

    assert_eq!(flipped.len(), 2, "an 0402 has two pads: {flipped:?}");
    for (mask, _) in &flipped {
        assert_eq!(
            mask & TOP,
            0,
            "a bottom part with a top-copper pad draws on the wrong side: {flipped:?}"
        );
        assert_ne!(mask & BOTTOM, 0, "and it has to be on the bottom");
    }
}

#[test]
fn a_part_left_alone_still_arrives_on_the_top() {
    // The control: every design in this repository states no side.
    let straight = pads(&snapshot(&board("")));

    for (mask, _) in &straight {
        assert_ne!(mask & TOP, 0, "{straight:?}");
        assert_eq!(mask & BOTTOM, 0, "{straight:?}");
    }
}

#[test]
fn the_geometry_is_flipped_rather_than_copied() {
    // Pad coordinates in a snapshot are relative to the part, not to the
    // board - the first version of this test subtracted the part's position
    // from them and compared -9.5mm against 10.5mm, which said nothing about
    // the flip and everything about the reader.
    //
    // Pad 1 of an 0402 sits left of centre; flipped, it sits right of centre.
    // If the two agree, the part was moved to the other layer without being
    // turned over, and anything with an asymmetric footprint gets soldered
    // mirrored.
    let flipped = pads(&snapshot(&board("    side bottom")));
    let straight = pads(&snapshot(&board("")));

    assert_eq!(
        straight[0].1, -500_000,
        "pad 1 of an 0402 is half a millimetre left of the part's centre"
    );
    assert_eq!(
        flipped[0].1, 500_000,
        "and half a millimetre right of it once the part is turned over"
    );
}
