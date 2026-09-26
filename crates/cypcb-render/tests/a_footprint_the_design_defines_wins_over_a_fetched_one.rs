//! A footprint the design defines wins over one the host fetched under the
//! same name, whichever arrived last.
//!
//! `cargo test -p cypcb-render --test a_footprint_the_design_defines_wins_over_a_fetched_one`
//!
//! `register_footprint` says so in its own documentation: a footprint the
//! design file defines itself still wins while it exists, and the fetched one
//! comes back when it goes. A load already built it that way. A fetch that
//! landed after the load did not: it replaced the design's footprint until the
//! next load, so the board changed shape under a design nobody had edited.

use cypcb_render::PcbEngine;

const PART: &str = "SHARED_NAME";

/// The design's own footprint for `PART`: one pad.
fn design_defining_it() -> String {
    format!(
        "version 1\n\nboard b {{\n    size 20mm x 20mm\n    layers 2\n}}\n\n\
         footprint {PART} {{\n    courtyard 3mm x 3mm\n    pad 1 rect at 0mm, 0mm size 1mm x 1mm\n}}\n\n\
         component U1 ic \"{PART}\" {{\n    at 10mm, 10mm\n}}\n"
    )
}

/// The same part, with no definition of its own.
fn design_using_it() -> String {
    format!(
        "version 1\n\nboard b {{\n    size 20mm x 20mm\n    layers 2\n}}\n\n\
         component U1 ic \"{PART}\" {{\n    at 10mm, 10mm\n}}\n"
    )
}

/// The fetched footprint for `PART`: three pads, so it cannot be mistaken
/// for the design's.
fn fetch(engine: &mut PcbEngine) {
    let pad = |number: &str, x_nm: i64| {
        format!(
            r#"{{"number":"{number}","x_nm":{x_nm},"y_nm":0,"width_nm":800000,"height_nm":800000,"shape":"rect","layer_mask":1,"drill_nm":null}}"#
        )
    };
    let pads = format!(
        "[{},{},{}]",
        pad("1", -2_000_000),
        pad("2", 0),
        pad("3", 2_000_000)
    );
    let refused = engine.register_footprint(PART, &pads, "");
    assert!(
        refused.is_empty(),
        "the engine took the footprint: {refused}"
    );
}

fn pads_on_u1(engine: &mut PcbEngine) -> usize {
    let snapshot: serde_json::Value =
        serde_json::from_str(&engine.get_snapshot()).expect("the engine writes JSON");
    snapshot["components"]
        .as_array()
        .expect("components")
        .iter()
        .find(|c| c["refdes"] == "U1")
        .expect("U1 is on the board")["pads"]
        .as_array()
        .expect("pads")
        .len()
}

#[test]
fn fetched_before_the_load_the_design_still_wins() {
    let mut engine = PcbEngine::new();
    fetch(&mut engine);
    let error = engine.load_source(&design_defining_it());
    assert!(error.is_empty(), "{error}");
    assert_eq!(pads_on_u1(&mut engine), 1);
}

#[test]
fn fetched_after_the_load_the_design_still_wins() {
    let mut engine = PcbEngine::new();
    let error = engine.load_source(&design_defining_it());
    assert!(error.is_empty(), "{error}");
    fetch(&mut engine);
    // Pads come from the library the snapshot is built with, which is the one
    // a fetch changes; a reload is not needed to see the difference.
    assert_eq!(pads_on_u1(&mut engine), 1);
    // The design did not change, so neither does its board on the next load.
    let error = engine.load_source(&design_defining_it());
    assert!(error.is_empty(), "{error}");
    assert_eq!(pads_on_u1(&mut engine), 1);
}

#[test]
fn once_the_design_drops_its_definition_the_fetched_one_is_used() {
    // The control: the fetched footprint is in the library, so the lines
    // above are the design winning and not the fetch failing.
    let mut engine = PcbEngine::new();
    let error = engine.load_source(&design_defining_it());
    assert!(error.is_empty(), "{error}");
    fetch(&mut engine);
    let error = engine.load_source(&design_using_it());
    assert!(error.is_empty(), "{error}");
    assert_eq!(pads_on_u1(&mut engine), 3);
}
