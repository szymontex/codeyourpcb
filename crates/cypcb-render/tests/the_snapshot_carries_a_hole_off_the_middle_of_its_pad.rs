//! The viewer is told where a pad's hole is when it is not in the middle.
//!
//! `cargo test -p cypcb-render --test the_snapshot_carries_a_hole_off_the_middle_of_its_pad`
//!
//! The snapshot hands the viewer each pad's copper and its drill, and the
//! viewer drew the drill in the middle of the land. A pad that states its hole
//! off the middle ([`PadDef::hole_offset`]) now carries it, in the footprint's
//! frame with y up and the pad's own turn taken up like its sides: the board's
//! one drill file and the screen then agree. A pad whose hole is in the middle
//! carries nothing, so a snapshot of every board that has none is the one it
//! always was.

use cypcb_render::PcbEngine;

const SOURCE: &str = r#"version 1

board fetched {
    size 30mm x 20mm
    layers 2
}

component J1 connector "FETCHED" {
    at 10mm, 10mm
}
"#;

/// Two pads: the first with its hole 0.5mm right and 0.25mm up, the second with
/// its hole in the middle.
const PADS: &str = r#"[
  {"number":"1","x_nm":0,"y_nm":0,"width_nm":2000000,"height_nm":3000000,"shape":"rect","layer_mask":3,"drill_nm":800000,"hole_offset_nm":[500000,250000]},
  {"number":"2","x_nm":2540000,"y_nm":0,"width_nm":2000000,"height_nm":3000000,"shape":"rect","layer_mask":3,"drill_nm":800000}
]"#;

fn snapshot() -> serde_json::Value {
    let mut engine = PcbEngine::new();
    assert_eq!(engine.register_footprint("FETCHED", PADS, "[]"), "");
    let errors = engine.load_source(SOURCE);
    assert!(errors.is_empty(), "{errors}");
    serde_json::from_str(&engine.get_snapshot()).expect("the snapshot is JSON")
}

#[test]
fn a_pad_with_its_hole_off_the_middle_says_so() {
    let snapshot = snapshot();
    let pads = &snapshot["components"][0]["pads"];
    assert_eq!(
        pads[0]["hole_offset_nm"],
        serde_json::json!([500_000, 250_000])
    );
}

#[test]
fn control_a_pad_with_its_hole_in_the_middle_says_nothing() {
    let snapshot = snapshot();
    let pad = &snapshot["components"][0]["pads"][1];
    assert!(pad.get("hole_offset_nm").is_none(), "{pad}");
}
