//! A command that promises JSON has to put JSON on stdout, and nothing else.
//!
//! `cargo test -p cypcb-cli --test what_a_command_says_on_stdout`
//!
//! `export --dry-run` printed its file list to stderr for as long as the flag
//! existed, so `> set.txt` wrote an empty file. That was found by hand. The
//! three commands whose whole output is machine-readable - `parse -o json`,
//! `parse-kicad` and `score` - were never checked at all, and the tests that
//! read them slice from the first `{` of stdout and stderr joined, which
//! passes just as well when a sentence of prose is sitting in the middle of
//! the stream a script parses.
//!
//! Here stdout is parsed whole. A stray `println!` anywhere in those commands
//! fails this, which is the point: prose belongs on the other stream.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Stdout alone, with stderr left where it is.
fn stdout_of(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "`cypcb {}` failed:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// The whole of stdout, as JSON.
fn json(args: &[&str]) -> serde_json::Value {
    let said = stdout_of(args);
    serde_json::from_str(said.trim()).unwrap_or_else(|error| {
        panic!(
            "`cypcb {}` should put JSON and nothing else on stdout: {error}\n{said}",
            args.join(" ")
        )
    })
}

#[test]
fn parse_prints_a_model_and_no_prose() {
    let model = json(&["parse", "-o", "json", "examples/blink.cypcb"]);
    assert!(
        model.is_object() || model.is_array(),
        "the model should be a JSON document: {model}"
    );
}

#[test]
fn parse_kicad_prints_metadata_and_no_prose() {
    let metadata = json(&[
        "parse-kicad",
        "tests/fixtures/benchmark/led_blink.kicad_pcb",
    ]);
    assert!(
        metadata.is_object(),
        "the metadata should be a JSON object: {metadata}"
    );
}

#[test]
fn score_prints_numbers_and_no_prose() {
    let scored = json(&["score", "examples/blink.cypcb"]);
    assert!(
        scored["drc_violations"].is_number(),
        "`score` counts DRC violations: {scored}"
    );
}
