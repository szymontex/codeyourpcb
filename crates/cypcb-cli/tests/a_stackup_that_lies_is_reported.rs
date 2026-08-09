//! The stackup a design writes has to describe the board beside it.
//!
//! `cargo test -p cypcb-cli --test a_stackup_that_lies_is_reported`
//!
//! `stackup { copper 0.035mm core 1.5mm copper 0.035mm }` parsed into
//! `BoardDef::stackup` and was read by nothing - the last construct in the
//! language in that state, and the one that goes to a fabricator. A board
//! could say `layers 4` and then describe two copper layers; the exporter
//! takes its Gerber count from `layers`, so the files and the build
//! instructions disagreed about what the board was and nothing said so.
//!
//! `cypcb-drc`'s own test covers the rule. This one covers the path: the
//! source text, both parsers, `sync_ast_to_world`, and the command a person
//! actually runs.

use std::process::Command;

fn board(layers: u32, stackup: &str) -> String {
    format!(
        r#"version 1

board panel {{
    size 30mm x 20mm
    layers {layers}
    stackup {{
{stackup}
    }}
}}

component R1 resistor "0402" {{
    value "10k"
    at 10mm, 10mm
}}
"#
    )
}

fn check(source: &str, name: &str) -> String {
    let dir = std::env::temp_dir().join("cypcb-layer-check");
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

const TWO_LAYER: &str = "        copper 0.035mm\n        core 1.5mm\n        copper 0.035mm";

#[test]
fn a_four_layer_board_with_a_two_layer_stackup_is_reported() {
    let report = check(&board(4, TWO_LAYER), "mismatch");

    assert!(
        report.contains("stackup at ("),
        "the board says four copper layers and describes two:\n{report}"
    );
    assert!(
        report.contains("4 copper layers") && report.contains("describes 2"),
        "the message has to carry both numbers:\n{report}"
    );
}

#[test]
fn a_stackup_that_agrees_with_the_board_is_silent() {
    // The control, and the reason this is safe to add to the default rule set:
    // a correct stackup reports nothing, and a design with no stackup at all -
    // which is every example in this repository - reports nothing either.
    let report = check(&board(2, TWO_LAYER), "agreed");

    // Matched against the violation line rather than against the word: the
    // first version of this looked for "stackup" anywhere in the output and
    // failed on a correct board, because the temporary file's own path
    // contained it. A control that fails for the wrong reason is not a
    // control.
    assert!(
        !report.contains("stackup at ("),
        "the stackup matches the board and something complained:\n{report}"
    );
}

#[test]
fn the_thickness_the_design_asks_for_is_in_the_report() {
    // 0.035 + 1.5 + 0.035. The tool does not judge it - it has no table of
    // what any fab will press - but the person reading the report does.
    let report = check(&board(4, TWO_LAYER), "thickness");

    assert!(report.contains("1.570mm of material"), "{report}");
}

#[test]
fn two_copper_layers_with_nothing_between_them_are_reported() {
    // Reaches the checker as a stackup rather than as a parse error: the
    // grammar accepts any order of layers, so this is a design that builds a
    // short and the only thing that can see it is the rule.
    let report = check(
        &board(
            2,
            "        copper 0.035mm\n        copper 0.035mm\n        core 1.5mm",
        ),
        "pressed",
    );

    assert!(
        report.contains("no dielectric between them"),
        "two foils pressed together are one foil:\n{report}"
    );
}
