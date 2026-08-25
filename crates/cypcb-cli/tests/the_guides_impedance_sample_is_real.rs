//! The figures the syntax guide prints in prose, measured.
//!
//! `cargo test -p cypcb-cli --test the_guides_impedance_sample_is_real`
//!
//! `docs/SYNTAX.md` shows what the checker says when a net asks for an
//! impedance its stack does not give: **22.29ohm**, **55.4% off**, and
//! **0.064mm would give 50ohm**. Three numbers from one run, and until this
//! test the guide did not say which board they came from - so a reader could
//! not reproduce them and nothing would notice when the arithmetic behind them
//! moved.
//!
//! They come from the four-layer stack `cypcb-fixtures` keeps for exactly this
//! purpose, whose four foils are all different thicknesses so that no two
//! copper layers answer alike, carrying `net SIG [impedance 50ohm]` on
//! `Inner1` at 0.2mm wide. The guide names that board now, and this reads the
//! figures back **out of the guide** and holds the command to them: the test
//! fails whether the document drifts or the forms do.

use std::path::{Path, PathBuf};
use std::process::Command;

use cypcb_fixtures::every_copper_layer_answers_differently_source;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// The sample as one line, however the guide wrapped it.
fn sample_from_the_guide() -> String {
    let guide = std::fs::read_to_string(repo_root().join("docs/SYNTAX.md"))
        .expect("the syntax guide is in the repo");
    let flat = guide.split_whitespace().collect::<Vec<_>>().join(" ");
    let start = flat
        .find("impedance: net 'SIG' asks for")
        .expect("the guide shows the impedance report");
    // The guide writes the kind as a short prefix - `impedance:` - where the
    // report writes the location and the trace first. What has to match is the
    // sentence, which starts at the net.
    let rest = &flat[start + "impedance: ".len()..];
    let end = rest.find("```").unwrap_or(rest.len());
    rest[..end].trim().to_string()
}

/// The same board the sample was taken on.
fn board() -> String {
    format!(
        r#"version 1

board named_layers {{
    size 30mm x 20mm
    layers 4
{stack}}}

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

net SIG [impedance 50ohm] {{
    J1.1
    J2.1
}}

trace SIG {{
    from J1.1
    to J2.1
    layer Inner1
    width 0.2mm
}}
"#,
        stack = every_copper_layer_answers_differently_source()
    )
}

#[test]
fn the_checker_still_says_what_the_guide_shows_it_saying() {
    let dir = std::env::temp_dir().join("cypcb-guide-impedance");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let file = dir.join("sample.cypcb");
    std::fs::write(&file, board()).expect("the board is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&file)
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    let flat = said.split_whitespace().collect::<Vec<_>>().join(" ");

    let sample = sample_from_the_guide();
    assert!(
        sample.contains("22.29ohm") && sample.contains("0.064mm"),
        "the sample the guide shows lost its figures: {sample}"
    );
    assert!(
        flat.contains(&sample),
        "the guide shows this and the checker does not say it:\n  guide: \
         {sample}\n  said:  {flat}"
    );
}
