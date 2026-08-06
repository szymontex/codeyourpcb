//! A footprint fetched at runtime has to become copper the checker can see.
//!
//! The viewer fetches parts from a supplier: they arrive as pads and silk with
//! no `footprint` block behind them, and `register_footprint` is the only road
//! into the board model. Until this fire the viewer never called it, so the
//! pads were drawn and the model held an unknown footprint - nothing to
//! measure clearance against and nothing to export.
//!
//! Wiring the call is not the same as the copper arriving. This checks the
//! second half: register a part, place two of them close enough to short, and
//! the checker has to say so.

use cypcb_render::PcbEngine;

/// Two pads on the top layer, an 0402 in all but name.
const PADS: &str = r#"[
    {"number":"1","shape":"rect","x_nm":-500000,"y_nm":0,"width_nm":600000,"height_nm":500000,"layer_mask":1},
    {"number":"2","shape":"rect","x_nm":500000,"y_nm":0,"width_nm":600000,"height_nm":500000,"layer_mask":1}
]"#;

fn board_with(gap_mm: f64) -> String {
    format!(
        r#"version 1

board fetched {{
    size 30mm x 20mm
    layers 2
}}

component R1 resistor "LCSC_C25804" {{
    value 10kohm
    at 10mm, 10mm
}}

component R2 resistor "LCSC_C25804" {{
    value 10kohm
    at {}mm, 10mm
}}

net SIG {{
    R1.2
    R2.1
}}
"#,
        10.0 + gap_mm
    )
}

#[test]
fn a_fetched_footprint_becomes_copper_the_checker_measures() {
    let mut engine = PcbEngine::new();

    let error = engine.register_footprint("LCSC_C25804", PADS, "");
    assert!(
        error.is_empty(),
        "the engine refused the footprint: {error}"
    );

    // Far apart: the parts clear each other and the board is clean.
    let error = engine.load_source(&board_with(5.0));
    assert!(error.is_empty(), "{error}");
    let far = engine.run_drc_incremental();

    // Close enough that R1's right pad and R2's left pad nearly touch. Pad
    // copper reaches 0.8mm either side of a part's centre, so at 1.7mm apart
    // the gap is 0.1mm - under the 0.127mm the fab preset asks for.
    let error = engine.load_source(&board_with(1.7));
    assert!(error.is_empty(), "{error}");
    let near = engine.run_drc_incremental();

    assert!(
        near > far,
        "moving two fetched parts into each other has to be visible to the checker: \
         {far} violations apart, {near} together"
    );

    let report = engine.get_violations_json();
    assert!(
        report.contains("clearance"),
        "the violation has to be a clearance one: {report}"
    );
}

#[test]
fn a_board_using_an_unregistered_footprint_is_refused_rather_than_passed() {
    // The failure this road exists to prevent is a part with no pads: every
    // rule that measures copper measures nothing and the board looks perfect.
    // The engine does better than that - it refuses the board and says which
    // footprint it does not know, which is the honest answer and worth pinning
    // so nobody softens it into a warning later.
    let mut engine = PcbEngine::new();

    let error = engine.load_source(&board_with(1.7));
    assert!(
        error.contains("LCSC_C25804"),
        "an unknown footprint has to be named, not skipped: {error:?}"
    );

    // Taught first, the same board loads.
    let mut taught = PcbEngine::new();
    assert!(taught
        .register_footprint("LCSC_C25804", PADS, "")
        .is_empty());
    assert!(
        taught.load_source(&board_with(1.7)).is_empty(),
        "the same board loads once the engine knows the part"
    );
}

#[test]
fn a_fetched_arc_becomes_ink_instead_of_being_dropped() {
    // A supplier's footprint often draws a rounded outline or a pin-one arc.
    // The board model has no arc, so one used to be parsed and thrown away:
    // the part arrived with its artwork and printed without it, silently. A
    // legend is ink rather than geometry anything reasons about, so an arc
    // becomes segments - which is what the exporter does with a circle anyway.
    use cypcb_render::SilkInfo;
    use cypcb_world::footprint::SilkShape;

    let quarter = SilkInfo::Arc {
        cx: 0,
        cy: 0,
        radius: 1_000_000,
        width: 150_000,
        start_angle: 0.0,
        end_angle: 90.0,
    };

    let shapes = quarter.to_shapes();
    assert_eq!(
        shapes.len(),
        8,
        "a quarter turn at 32 segments to the circle is eight of them"
    );
    assert!(
        shapes
            .iter()
            .all(|shape| matches!(shape, SilkShape::Segment { .. })),
        "an arc becomes segments, not circles"
    );

    // It starts on the +X axis and ends on +Y, at the radius it was given.
    let SilkShape::Segment { start, .. } = shapes[0] else {
        panic!("the first shape is a segment");
    };
    assert_eq!((start.x.0, start.y.0), (1_000_000, 0));
    let SilkShape::Segment { end, .. } = shapes[7] else {
        panic!("the last shape is a segment");
    };
    assert_eq!((end.x.0, end.y.0), (0, 1_000_000));

    // An arc that says nothing about its angles is a full circle, which is
    // what the payloads written before arcs carried angles meant.
    let whole = SilkInfo::Arc {
        cx: 0,
        cy: 0,
        radius: 1_000_000,
        width: 150_000,
        start_angle: 0.0,
        end_angle: 0.0,
    };
    assert_eq!(whole.to_shapes().len(), 32, "a full turn is 32 segments");
}
