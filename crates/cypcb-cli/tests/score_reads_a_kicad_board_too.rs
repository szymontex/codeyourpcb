//! Every command that takes a board takes both kinds of board.
//!
//! `cargo test -p cypcb-cli --test score_reads_a_kicad_board_too`
//!
//! `check`, `export` and `route` go through `board_source`, which decides
//! which reader a file gets. `score` did not, so it handed a `(kicad_pcb ...)`
//! file to the DSL parser and printed thirty-five diagnostics, each of them
//! wrong about a file that is not:
//!
//! ```text
//!   × Missing a net name
//!     ╭─[36:8]
//!  36 │   (net 1 "VCC")
//!     ·        ┬
//!     ·        ╰── expected a net name
//! ```
//!
//! The benchmark fixtures this project measures its router with are all KiCad
//! files. The one command whose whole job is to put a number on a routed board
//! could not read a single one of them.
//!
//! `parse` is the other half and gets the opposite treatment: it reads the
//! `.cypcb` language and there is a `parse-kicad` for the other, so it says so
//! in one line instead of a page.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark")
        .join(name)
}

fn run(args: &[&str]) -> (bool, String, String) {
    let output = cypcb().args(args).output().expect("the binary runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn score_puts_a_number_on_a_kicad_board() {
    let board = fixture("led_blink.kicad_pcb");
    let (ok, stdout, stderr) = run(&["score", board.to_str().expect("a utf-8 path")]);

    assert!(ok, "scoring a KiCad board has to work:\n{stderr}");
    let score: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("score has to print JSON ({e}):\n{stdout}\n{stderr}"));

    for metric in [
        "total_length",
        "via_count",
        "drc_violations",
        "smoothness",
        "shorts",
        "crossings",
        "layer_balance",
        "composite",
    ] {
        assert!(
            score.get(metric).is_some(),
            "the score is missing {metric}: {score}"
        );
    }
    assert!(
        score["total_length"].as_i64().unwrap_or(0) > 0,
        "this board carries copper, so its length is not zero: {score}"
    );
}

#[test]
fn score_and_check_agree_about_the_same_kicad_board() {
    // The reason this matters. A number nobody can reproduce with another
    // command is a number nobody should trust, and `score` used to answer for
    // a fabricator nobody named - fixed once already for `.cypcb` boards.
    let board = fixture("led_blink.kicad_pcb");
    let path = board.to_str().expect("a utf-8 path");

    let (_, stdout, _) = run(&["score", path, "--preset", "jlcpcb"]);
    let score: Value = serde_json::from_str(&stdout).expect("score prints JSON");
    let scored = score["drc_violations"].as_i64().expect("a violation count");

    let (_, check_out, check_err) = run(&["check", path, "--preset", "jlcpcb"]);
    let both = format!("{check_out}{check_err}");
    let reported: i64 = both
        .lines()
        .find_map(|line| line.split_whitespace().next()?.parse().ok())
        .unwrap_or_else(|| panic!("check has to open with a count:\n{both}"));

    assert_eq!(
        scored, reported,
        "score says {scored} and check says {reported} about one board:\n{both}"
    );
}

#[test]
fn parse_says_which_command_reads_a_kicad_board() {
    let board = fixture("led_blink.kicad_pcb");
    let (ok, stdout, stderr) = run(&["parse", board.to_str().expect("a utf-8 path")]);
    let both = format!("{stdout}{stderr}");

    assert!(!ok, "the DSL parser cannot read this file:\n{both}");
    assert!(
        both.contains("parse-kicad"),
        "the refusal has to name the command that can read it:\n{both}"
    );
    assert!(
        !both.contains("Missing a net name"),
        "a valid KiCad net is not a missing net name:\n{both}"
    );
    assert!(
        both.lines().count() < 15,
        "one clear line, not a page of diagnostics - got {} lines:\n{both}",
        both.lines().count()
    );
}

#[test]
fn the_weights_option_that_did_nothing_is_gone() {
    // It was hidden from `--help` and documented as future-proofing, and the
    // scoring call has always used `ScoreWeights::default()`. An option that
    // is accepted and ignored is worse than one that does not exist.
    let board = fixture("led_blink.kicad_pcb");
    let (ok, stdout, stderr) = run(&[
        "score",
        board.to_str().expect("a utf-8 path"),
        "--weights",
        "1,1,1",
    ]);
    let both = format!("{stdout}{stderr}");

    assert!(!ok, "an option nothing reads has to be rejected:\n{both}");
    assert!(
        both.contains("--weights") || both.contains("unexpected argument"),
        "and clap has to say which argument it did not know:\n{both}"
    );
}
