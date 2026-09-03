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

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

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
    // `watch` is not in this loop because it does not return; it is asked the
    // same question by the case at the end of this file, which reads its first
    // pass and then kills it.
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
fn watch_reads_the_kicad_board_it_is_pointed_at() {
    // This used to grep `watch.rs` for `is_kicad(`, because a command that
    // never returns cannot be run and waited for. That is the weaker check
    // this file exists to replace: the same grep would pass on `parse`, which
    // calls `is_kicad` in order to refuse.
    //
    // A watch can be asked after all. It prints one check before it starts
    // watching, so the run is given a board KiCad wrote, read until that first
    // verdict appears, and killed. A verdict is proof the file was understood;
    // a KiCad board read as the .cypcb language cannot produce one.
    let board = kicad_board("watch");
    let mut child = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("watch")
        .arg(&board)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");

    let stdout = child.stdout.take().expect("stdout was piped");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                return;
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(60);
    let mut said = Vec::new();
    let mut verdict = None;
    while Instant::now() < deadline && verdict.is_none() {
        match receiver.recv_timeout(Duration::from_millis(500)) {
            Ok(line) => {
                if line.starts_with("OK:") || line.contains("DRC violation(s) against") {
                    verdict = Some(line.clone());
                }
                said.push(line);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        verdict.is_some(),
        "`watch` had to check the KiCad board it was handed and say so; it said:\n{}",
        said.join("\n")
    );
    assert!(
        help_line("watch").contains(BOTH_FORMATS),
        "and its help line has to say it reads one: {}",
        help_line("watch")
    );
}
