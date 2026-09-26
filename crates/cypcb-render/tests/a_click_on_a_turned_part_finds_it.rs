//! A click on a turned part's pad finds the part.
//!
//! `cargo test -p cypcb-render --features native --test a_click_on_a_turned_part_finds_it`
//!
//! The viewer asks the spatial index what is under the pointer. A part sits in
//! the index as its courtyard, and the viewer fills the index two ways: from
//! the footprint library when a board loads, and piece by piece after an edit.
//! Both have to turn the courtyard with the part, or a click on the pad of a
//! part turned 90 degrees lands where the index has nothing.
//!
//! The part is a bar: two pads 6mm apart on a courtyard 7mm by 1mm, so turned
//! 90 or 45 degrees neither pad is inside the box left unturned.

use cypcb_render::PcbEngine;

const ANGLES: [f64; 3] = [0.0, 90.0, 45.0];

fn board(degrees: f64) -> String {
    format!(
        r#"version 1

footprint BAR {{
    description "two pads 6mm apart"
    courtyard 7mm x 1mm

    pad 1 rect at -3mm, 0mm size 0.6mm x 0.6mm
    pad 2 rect at 3mm, 0mm size 0.6mm x 0.6mm
}}

board turned {{
    size 20mm x 20mm
    layers 2
}}

component U1 ic "BAR" {{
    value "bar"
    at 10mm, 10mm
    rotate {degrees}
}}
"#
    )
}

/// Where pad `x_mm` along the bar lands, in nanometres.
fn pad_at(degrees: f64, x_mm: f64) -> (i64, i64) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    (
        ((10.0 + x_mm * cos) * 1e6).round() as i64,
        ((10.0 + x_mm * sin) * 1e6).round() as i64,
    )
}

/// The pads a click does not find the part at, as (degrees, x, y) in nm.
fn missed(engine: &mut PcbEngine, degrees: f64) -> Vec<(f64, i64, i64)> {
    [-3.0, 3.0]
        .into_iter()
        .map(|x| pad_at(degrees, x))
        .filter(|&(x, y)| !engine.query_point(x, y).contains(&"U1".to_string()))
        .map(|(x, y)| (degrees, x, y))
        .collect()
}

#[test]
fn a_click_on_each_pad_finds_the_part_once_the_board_is_loaded() {
    let mut lost = Vec::new();
    for degrees in ANGLES {
        let mut engine = PcbEngine::new();
        engine.load_source(&board(degrees));
        lost.extend(missed(&mut engine, degrees));
    }
    assert!(lost.is_empty(), "no part under these pads: {lost:?}");
}

#[test]
fn a_click_on_each_pad_finds_the_part_after_an_edit() {
    // Adding a trace rebuilds the index the other way, piece by piece. The
    // trace runs across pad 2, and a click on it has to find the trace too.
    let mut lost = Vec::new();
    let mut unpicked = Vec::new();
    for degrees in ANGLES {
        let mut engine = PcbEngine::new();
        engine.load_source(&board(degrees));
        let (x, y) = pad_at(degrees, 3.0);
        let trace = engine.add_trace("N", "Top", 100_000, &[x - 1_000_000, y, x + 1_000_000, y]);
        assert_ne!(trace, u32::MAX, "the trace is added");
        lost.extend(missed(&mut engine, degrees));
        if engine.get_trace_at_point(x, y, 10_000) != trace {
            unpicked.push(degrees);
        }
    }
    assert!(lost.is_empty(), "no part under these pads: {lost:?}");
    assert!(
        unpicked.is_empty(),
        "the trace over pad 2 is not picked at: {unpicked:?}"
    );
}
