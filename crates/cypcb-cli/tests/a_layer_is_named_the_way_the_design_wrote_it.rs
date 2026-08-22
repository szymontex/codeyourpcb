//! A layer is named in a report the way the design named it.
//!
//! `cargo test -p cypcb-cli --test a_layer_is_named_the_way_the_design_wrote_it`
//!
//! `Layer::Inner` is zero-based and the language is one-based, so the first
//! inner layer is `Inner(0)` in this codebase and `Inner1` in a source file.
//! The impedance rule formatted the layer with `{:?}` and told a designer who
//! had written `layer Inner1` that the fault was on `Inner(0)` - a spelling
//! that is not in the language, carrying a number their file never uses. The
//! top layer read `TopCopper` for what the language calls `Top`.
//!
//! This runs the built binary rather than the rule, because the second half of
//! what it holds is that the rule speaks about an inner layer at all. That was
//! open in the tracker as "a trace on an inner layer reads as no copper at
//! all", observed and not diagnosed: the cause was the `copper_index`
//! off-by-one, fixed since, and nothing had run the whole path to say so.

use cypcb_fixtures::{
    an_inner_layer_the_forms_cannot_describe_source, every_copper_layer_answers_differently_source,
};
use std::process::Command;

/// A four-layer board with a controlled-impedance net on one layer.
///
/// The stack comes from `cypcb-fixtures` rather than from this file. The
/// fixture crate exists because three shipped index errors were all hidden by
/// a symmetric stack, and until now it held its stacks as `Stackup` values,
/// which a test driving the command line cannot use - so this test built its
/// own, which is the habit the crate was written to end.
///
/// The pads are drilled on purpose. A 0402 lands copper on the top layer only,
/// so a trace on an inner layer cannot reach it and the board's complaint is
/// about connectivity instead of impedance - which is what made the original
/// observation ambiguous.
fn board(stackup: &str, layer: &str, target: &str) -> String {
    format!(
        r#"version 1

board named_layers {{
    size 30mm x 20mm
    layers 4
{stackup}}}

footprint PAD1 {{
    description "one square pad, drilled so it reaches every layer"
    courtyard 2mm x 2mm
    pad 1 rect at 0mm, 0mm size 1.6mm x 1.6mm drill 0.8mm
}}

component J1 connector "PAD1" {{
    value "in"
    at 5mm, 10mm
}}

component J2 connector "PAD1" {{
    value "out"
    at 25mm, 10mm
}}

net SIG [impedance {target}] {{
    J1.1
    J2.1
}}

trace SIG {{
    from J1.1
    to J2.1
    layer {layer}
    width 0.2mm
}}
"#
    )
}

fn check(source: &str, name: &str) -> String {
    let dir = std::env::temp_dir().join("cypcb-layer-naming");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let file = dir.join(format!("{name}.cypcb"));
    std::fs::write(&file, source).expect("the board is written");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check"])
        .arg(&file)
        .output()
        .expect("the binary runs");

    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The impedance a report states, in hundredths of an ohm.
///
/// Read back out of the message rather than computed, because what this file
/// is about is what a person is told.
fn ohms_x100(report: &str) -> u32 {
    let after = report.split(" gives ").nth(1).unwrap_or_else(|| {
        panic!("no impedance in the report:\n{report}");
    });
    let figure = after.split("ohm").next().expect("a number before `ohm`");
    let (whole, hundredths) = figure.split_once('.').expect("a number like 41.16");
    whole.trim().parse::<u32>().expect("whole ohms") * 100
        + hundredths.parse::<u32>().expect("hundredths")
}

#[test]
fn an_inner_layer_is_called_what_the_source_calls_it() {
    let stack = every_copper_layer_answers_differently_source();
    let report = check(&board(&stack, "Inner1", "90ohm"), "inner");

    assert!(
        report.contains("on Inner1 gives"),
        "the design wrote `layer Inner1`, so the report says Inner1:\n{report}"
    );
    assert!(
        !report.contains("Inner(0)") && !report.contains("Inner 0"),
        "neither the internal spelling nor its zero-based number belongs in a report:\n{report}"
    );
}

#[test]
fn no_two_copper_layers_of_the_shared_fixture_answer_alike() {
    // The fixture's whole promise, held through the binary rather than through
    // a unit test: four copper layers, four different answers. A rule reading
    // the wrong layer index cannot produce the right number on this board,
    // which is what a symmetric stack let three shipped index errors do.
    //
    // Asking for 1 ohm forces every layer to report, since nothing this stack
    // builds is anywhere near it.
    let stack = every_copper_layer_answers_differently_source();
    let mut said: Vec<(&str, u32)> = Vec::new();
    for layer in ["Top", "Inner1", "Inner2", "Bottom"] {
        let report = check(&board(&stack, layer, "1ohm"), &format!("all-{layer}"));
        assert!(
            report.contains(&format!("on {layer} gives")),
            "the report names the layer the design wrote:\n{report}"
        );
        said.push((layer, ohms_x100(&report)));
    }

    for (index, (layer, ohms)) in said.iter().enumerate() {
        for (other_layer, other_ohms) in said.iter().skip(index + 1) {
            assert_ne!(
                ohms, other_ohms,
                "{layer} and {other_layer} answer alike, so this board cannot tell them apart: {said:?}"
            );
        }
    }
}

#[test]
fn the_top_layer_is_called_top() {
    let stack = every_copper_layer_answers_differently_source();
    let report = check(&board(&stack, "Top", "90ohm"), "top");

    assert!(
        report.contains("on Top gives"),
        "the language calls it `Top`:\n{report}"
    );
    assert!(
        !report.contains("TopCopper"),
        "`TopCopper` is the internal name:\n{report}"
    );
}

#[test]
fn the_bottom_layer_is_called_bottom() {
    // The bottom layer is here because it is where the second of the three
    // index errors was: `BottomCopper` read as copper entry 0.
    let stack = every_copper_layer_answers_differently_source();
    let report = check(&board(&stack, "Bottom", "90ohm"), "bottom");

    assert!(
        report.contains("on Bottom gives"),
        "the language calls it `Bottom`:\n{report}"
    );
    assert!(
        !report.contains("BottomCopper"),
        "`BottomCopper` is the internal name:\n{report}"
    );
}

#[test]
fn the_layer_a_stack_cannot_describe_is_named_the_same_way() {
    // The rule's other message carries a layer too, and it was formatted the
    // same wrong way. The stack is the second fixture: prepreg outside, thick
    // core in the middle, so neither inner layer is a form the closed
    // solutions cover.
    let stack = an_inner_layer_the_forms_cannot_describe_source();
    let report = check(&board(&stack, "Inner1", "50ohm"), "lopsided");

    assert!(
        report.contains("delivers on Inner1:"),
        "the message that declines to measure names the layer as well:\n{report}"
    );
    assert!(
        report.contains("Not checked - not passed"),
        "declining to measure is not a pass:\n{report}"
    );
}
