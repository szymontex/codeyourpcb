//! A flag the run cannot honour is refused by name, not swallowed.
//!
//! `cargo test -p cypcb-cli --test a_flag_that_cannot_be_honoured_is_refused`
//!
//! `cypcb route` has three ways to route a board: FreeRouting for a `.cypcb`
//! file, the built-in router for the same file with `--in-house`, and the
//! built-in router for every `.kicad_pcb` file. Four of the command's options
//! belong to FreeRouting alone, and the other two paths accepted them and did
//! nothing about them.
//!
//! `--dry-run` is the one that shows what that costs. It says "export DSN
//! only, don't run FreeRouting", and measured on a KiCad board before this
//! change it routed the whole board and wrote it out:
//!
//! ```text
//! $ cypcb route led_blink.kicad_pcb --dry-run --fast
//! Routing led_blink.kicad_pcb...
//! Wrote led_blink.routed.kicad_pcb (21 segments, 4 vias) in 0.03s
//! ```
//!
//! A user who typed `--dry-run` to see the DSN got a routed board instead, and
//! nothing said the flag had been ignored.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A copy of a board in a directory of this test's own.
fn scratch_copy(from: &str, who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-refused-flags-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let source = repo_root().join(from);
    let target = dir.join(source.file_name().expect("the fixture has a name"));
    std::fs::copy(&source, &target).expect("the fixture is copyable");
    target
}

fn run(board: &Path, flags: &[&str]) -> (bool, String) {
    let output = cypcb()
        .arg("route")
        .arg(board)
        .args(flags)
        .output()
        .expect("the binary runs");
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

#[test]
fn dry_run_on_a_kicad_board_is_refused_and_writes_nothing() {
    let board = scratch_copy("tests/fixtures/benchmark/led_blink.kicad_pcb", "kicad-dry");
    let (ok, out) = run(&board, &["--dry-run", "--fast"]);

    assert!(
        !ok,
        "the flag cannot be honoured, so this is not a success:\n{out}"
    );
    assert!(
        out.contains("--dry-run"),
        "the refusal has to name the flag it is about:\n{out}"
    );
    assert!(
        out.contains("in-house"),
        "and say why this run cannot honour it:\n{out}"
    );
    assert!(
        !board.with_extension("routed.kicad_pcb").exists(),
        "a refused run must not leave a routed board behind"
    );
}

#[test]
fn the_in_house_router_refuses_the_freerouting_options_too() {
    // Same defect, other path: `--in-house` returns before any of these are
    // read.
    let board = scratch_copy("examples/blink.cypcb", "in-house-passes");
    let (ok, out) = run(&board, &["--in-house", "--fast", "--max-passes", "5"]);

    assert!(
        !ok,
        "--max-passes is FreeRouting's, not the built-in router's:\n{out}"
    );
    assert!(
        out.contains("--max-passes"),
        "the refusal has to name the flag:\n{out}"
    );
}

#[test]
fn several_refused_flags_are_all_named() {
    let board = scratch_copy("tests/fixtures/benchmark/led_blink.kicad_pcb", "kicad-both");
    let (ok, out) = run(&board, &["--dry-run", "--max-passes", "3", "--fast"]);

    assert!(!ok, "still a refusal:\n{out}");
    assert!(
        out.contains("--dry-run") && out.contains("--max-passes"),
        "a user who typed two ignored flags has to hear about both:\n{out}"
    );
}

#[test]
fn a_run_without_those_flags_is_untouched() {
    // The half that must not change: the flags that do apply still work, on
    // both paths.
    let kicad = scratch_copy(
        "tests/fixtures/benchmark/led_blink.kicad_pcb",
        "kicad-clean",
    );
    let (ok, out) = run(&kicad, &["--fast"]);
    assert!(ok, "routing a KiCad board still has to work:\n{out}");
    assert!(
        kicad.with_extension("routed.kicad_pcb").exists(),
        "and still has to write the board:\n{out}"
    );

    let dsl = scratch_copy("examples/blink.cypcb", "in-house-clean");
    let (ok, out) = run(&dsl, &["--in-house", "--fast"]);
    assert!(ok, "the in-house router still has to work:\n{out}");
}
