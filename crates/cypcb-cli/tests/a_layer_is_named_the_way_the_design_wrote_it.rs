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

use std::process::Command;

/// A four-layer board with a controlled-impedance net on one layer.
///
/// The pads are drilled on purpose. A 0402 lands copper on the top layer only,
/// so a trace on an inner layer cannot reach it and the board's complaint is
/// about connectivity instead of impedance - which is what made the original
/// observation ambiguous.
fn board(layer: &str, dielectric_below_the_first_inner: &str, target: &str) -> String {
    format!(
        r#"version 1

board named_layers {{
    size 30mm x 20mm
    layers 4
    stackup {{
        copper 0.035mm
        prepreg 0.3mm dk 4.5
        copper 0.0175mm
{dielectric_below_the_first_inner}
        copper 0.0175mm
        core 1.095mm dk 4.5
        copper 0.035mm
    }}
}}

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

/// The first inner layer sits between two equal prepregs: a stripline the
/// closed form covers, so the rule returns a number rather than declining.
const CENTRED: &str = "        prepreg 0.3mm dk 4.5";

/// The ordinary build - prepreg above, thick core below - which no form here
/// covers. The rule says so, and that message names a layer too.
const LOPSIDED: &str = "        core 1.095mm dk 4.5";

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

#[test]
fn an_inner_layer_is_called_what_the_source_calls_it() {
    let report = check(&board("Inner1", CENTRED, "90ohm"), "inner-centred");

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
fn the_rule_measures_a_trace_on_an_inner_layer() {
    // The half of this that is not about spelling. A centred stripline at
    // 0.200mm on this stack is 52.61 ohm; asking for 90 forces the number into
    // the report, where asking for 50 would pass and print nothing.
    let report = check(&board("Inner1", CENTRED, "90ohm"), "inner-measured");

    assert!(
        report.contains("52.61ohm"),
        "an inner layer is measured, not skipped:\n{report}"
    );
}

#[test]
fn the_top_layer_is_called_top() {
    // The same trace one layer up is a microstrip, and a different number, so
    // this also holds the two layers apart: a fixture whose layers all agree
    // cannot catch an index error.
    let report = check(&board("Top", CENTRED, "90ohm"), "top");

    assert!(
        report.contains("on Top gives") && report.contains("79.42ohm"),
        "the language calls it `Top`, and the top layer is a microstrip:\n{report}"
    );
    assert!(
        !report.contains("TopCopper"),
        "`TopCopper` is the internal name:\n{report}"
    );
}

#[test]
fn the_layer_a_stack_cannot_describe_is_named_the_same_way() {
    // The other message in the rule carries a layer too, and it was formatted
    // the same wrong way.
    let report = check(&board("Inner1", LOPSIDED, "50ohm"), "inner-lopsided");

    assert!(
        report.contains("delivers on Inner1:"),
        "the message that declines to measure names the layer as well:\n{report}"
    );
    assert!(
        report.contains("Not checked - not passed"),
        "declining to measure is not a pass:\n{report}"
    );
}
