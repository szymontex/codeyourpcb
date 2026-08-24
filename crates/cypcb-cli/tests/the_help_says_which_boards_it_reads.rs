//! The help says which boards a command reads, and it is asked rather than read.
//!
//! `cargo test -p cypcb-cli --test the_help_says_which_boards_it_reads`
//!
//! Six subcommands call `board_source::is_kicad`, and the first version of
//! this test paired that call with a help line naming both formats. It was
//! wrong the day it was written: `parse` calls it to **refuse** - "`parse`
//! reads the .cypcb language and this is a KiCad board" - so a source grep
//! cannot tell support from detection, and the line it made true said `parse`
//! reads a board it turns away.
//!
//! So each command is handed a board KiCad itself wrote and asked. What it
//! does with the file is the fact; the help line is checked against that.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The phrase a dual-format command's help line carries.
const BOTH_FORMATS: &str = ".cypcb or .kicad_pcb";

/// What a command says when it will not read a KiCad board.
const REFUSAL: &str = "reads the .cypcb language and this is a KiCad board";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A board KiCad 10.0.5 wrote, copied so nothing lands in the repo.
fn kicad_board(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-reads-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let source = repo_root().join("crates/cypcb-kicad/tests/fixtures/kicad10-slotted.kicad_pcb");
    let target = dir.join("board.kicad_pcb");
    std::fs::copy(&source, &target).expect("the fixture is copyable");
    target
}

/// Whether the command took the board rather than turning it away.
///
/// Not whether it succeeded: a command can accept a KiCad board and then fail
/// on it for its own reasons, which is a different fault from refusing to read
/// the format at all.
fn takes_a_kicad_board(subcommand: &str) -> bool {
    let board = kicad_board(subcommand);
    let out = board.parent().expect("a directory").join("out");
    let mut command = Command::new(env!("CARGO_BIN_EXE_cypcb"));
    command.arg(subcommand).arg(&board);
    if subcommand == "export" {
        command.arg("-o").arg(&out);
    }
    let output = command.output().expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stderr).to_string();
    !said.contains(REFUSAL)
}

/// The line clap prints for a subcommand in `cypcb --help`.
fn help_line(subcommand: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("--help")
        .output()
        .expect("the binary runs");
    let help = String::from_utf8_lossy(&output.stdout).to_string();
    help.lines()
        .find(|line| line.trim_start().starts_with(subcommand))
        .unwrap_or_else(|| panic!("no line for `{subcommand}` in:\n{help}"))
        .to_string()
}

#[test]
fn the_help_line_matches_what_the_command_does() {
    // `watch` is not here and cannot be: it does not return. Its help line is
    // held by the reader below instead, which is the weaker check this test
    // exists to replace everywhere it can.
    for subcommand in ["parse", "check", "export", "score"] {
        let takes = takes_a_kicad_board(subcommand);
        let line = help_line(subcommand);
        let says = line.contains(BOTH_FORMATS);

        assert_eq!(
            takes,
            says,
            "`{subcommand}` {} a KiCad board and its help line {} say so: {line}",
            if takes { "takes" } else { "refuses" },
            if says { "does" } else { "does not" },
        );
    }
}

#[test]
fn parse_is_the_one_that_turns_a_kicad_board_away() {
    // Stated on its own, because it is the case that made the source-reading
    // version of this test wrong. `parse` detects the format in order to
    // refuse it, and points at the command that does read it.
    let board = kicad_board("parse-refusal");
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("parse")
        .arg(&board)
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(!output.status.success(), "it has to fail, not half-read it");
    assert!(said.contains(REFUSAL), "{said}");
    assert!(
        said.contains("parse-kicad"),
        "and it has to name the command that does read one: {said}"
    );
}

#[test]
fn watch_is_the_one_command_this_can_only_read() {
    // It watches a file forever, so it cannot be handed a board and asked.
    // The source is the fallback: it opens with the same format check the
    // others do, and its line says both formats.
    let body = std::fs::read_to_string(repo_root().join("crates/cypcb-cli/src/commands/watch.rs"))
        .expect("watch.rs is readable");
    assert!(
        body.contains("is_kicad("),
        "watch checks the format it was handed"
    );
    assert!(
        help_line("watch").contains(BOTH_FORMATS),
        "and says so: {}",
        help_line("watch")
    );
}
