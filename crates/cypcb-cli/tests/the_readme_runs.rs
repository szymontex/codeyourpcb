//! The README, run rather than read.
//!
//! `cargo test -p cypcb-cli --test the_readme_runs`
//!
//! The CLI block lists eleven commands and each carries a claim in a trailing
//! comment. They were run once, by a person, on one day. Two had gone stale by
//! the time this was written: `from-kicad board.kicad_pcb` named a file the
//! repository does not have and answered `I/O error: No such file or
//! directory`, and the sentence under the block said a routed board is
//! **27 violations** to `check --preset pcbway` and 27 to `score` - it is 4
//! and 4.
//!
//! What this holds:
//!
//! - every command in the block runs, from a copy of the repository, and ends
//!   with the status its comment implies (`exit 1 on violations` means 1);
//! - a figure a comment states - `14 manufacturing files` - is a figure the
//!   command prints;
//! - the two claims the prose makes about numbers are read out of the README
//!   itself and measured, so the test fails whether the document drifts or the
//!   tool does.
//!
//! `watch` is the one command not run: it waits for a file to change and would
//! never return. It is named here rather than skipped quietly.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn readme() -> String {
    std::fs::read_to_string(repo_root().join("README.md")).expect("the README is where it was")
}

/// Everything the README's CLI block asks the reader to type.
fn commands() -> Vec<(Vec<String>, String)> {
    readme()
        .lines()
        .filter_map(|line| line.strip_prefix("cargo run -p cypcb-cli -- "))
        .map(|rest| {
            let (command, claim) = match rest.split_once('#') {
                Some((command, claim)) => (command, claim.trim().to_string()),
                None => (rest, String::new()),
            };
            (
                command.split_whitespace().map(str::to_string).collect(),
                claim,
            )
        })
        .collect()
}

/// A copy of what the commands need, so nothing writes into the repository.
///
/// `route` leaves a `.routed.cypcb` beside its input and `from-kicad` leaves a
/// `.cypcb` beside the board it read.
fn workspace(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-readme-run-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("examples/lib")).expect("a place to work");
    std::fs::create_dir_all(dir.join("tests/fixtures/benchmark")).expect("a place to work");

    copy_tree(&repo_root().join("examples"), &dir.join("examples"));
    let board = "tests/fixtures/benchmark/led_blink.kicad_pcb";
    std::fs::copy(repo_root().join(board), dir.join(board)).expect("the KiCad fixture is there");
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).expect("a directory to copy") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            std::fs::create_dir_all(&target).expect("a place to work");
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("a file to copy");
        }
    }
}

fn run(args: &[String], cwd: &Path) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|err| panic!("`cypcb {}` did not run: {err}", args.join(" ")));
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    (output.status.code(), said)
}

#[test]
fn every_command_the_readme_lists_runs_and_ends_the_way_it_says() {
    let commands = commands();
    assert!(
        commands.len() >= 11,
        "eleven commands were listed when this was written: {} found",
        commands.len()
    );
    let dir = workspace("commands");

    let mut skipped = Vec::new();
    for (args, claim) in &commands {
        if args.first().map(String::as_str) == Some("watch") {
            // Waits for the file to change and never returns.
            skipped.push(args.join(" "));
            continue;
        }

        let (code, said) = run(args, &dir);
        let wanted = if claim.contains("exit 1") { 1 } else { 0 };
        assert_eq!(
            code,
            Some(wanted),
            "`cypcb {}` is in the README as `{claim}` and exited {code:?}:\n{said}",
            args.join(" ")
        );

        // A number in the comment is a claim about what the command prints.
        if let Some(count) = claim
            .split_whitespace()
            .next()
            .and_then(|word| word.parse::<usize>().ok())
        {
            assert!(
                said.contains(&format!("{count} files")),
                "the README says `{claim}` and the command does not print that \
                 many:\n{said}"
            );
        }
    }

    assert_eq!(
        skipped,
        vec!["watch examples/blink.cypcb".to_string()],
        "only `watch` waits forever; anything else has to be run"
    );
}

/// The pair of numbers a sentence states about one file.
///
/// `\u{60}examples/blink.cypcb\u{60} is 24 violations to \u{60}check\u{60} and 9 to \u{60}score\u{60}` -> (24, 9).
fn pair_stated_about(flat: &str, file: &str) -> (usize, usize) {
    let tail = flat
        .split_once(&format!("`{file}` is "))
        .unwrap_or_else(|| panic!("the README says nothing about {file}"))
        .1;
    let first: usize = tail
        .split_whitespace()
        .next()
        .and_then(|word| word.parse().ok())
        .unwrap_or_else(|| panic!("no count after `{file}` is"));
    let second: usize = tail
        .split_once(" and ")
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .and_then(|word| word.parse().ok())
        .unwrap_or_else(|| panic!("no second count in the sentence about {file}"));
    (first, second)
}

#[test]
fn the_two_numbers_the_prose_states_are_what_the_commands_print() {
    let flat = readme().split_whitespace().collect::<Vec<_>>().join(" ");
    let dir = workspace("prose");

    // Unrouted: `score` routes the board before measuring it, so the two do
    // not agree and the README says why.
    let (checked, scored) = pair_stated_about(&flat, "examples/blink.cypcb");
    let (_, said) = run(&["check".into(), "examples/blink.cypcb".into()], &dir);
    assert!(
        said.contains(&format!("{checked} DRC violation(s)")),
        "the README says `check` calls that board {checked} violations:\n{said}"
    );
    let (_, said) = run(&["score".into(), "examples/blink.cypcb".into()], &dir);
    assert!(
        said.contains(&format!("\"drc_violations\": {scored}")),
        "and `score` {scored}, having laid its own copper first:\n{said}"
    );

    // Routed: the same board through both, against the same house, has to
    // agree - that is the claim the sentence exists to make.
    let routed = "examples/blink.routed.cypcb";
    let (with_check, with_score) = pair_stated_about(&flat, routed);
    assert_eq!(
        with_check, with_score,
        "the sentence is about the two agreeing, so the two numbers in it have \
         to be the same"
    );
    run(
        &[
            "route".into(),
            "examples/blink.cypcb".into(),
            "--variants".into(),
        ],
        &dir,
    );
    let (_, said) = run(
        &[
            "check".into(),
            "--preset".into(),
            "pcbway".into(),
            routed.into(),
        ],
        &dir,
    );
    assert!(
        said.contains(&format!("{with_check} DRC violation(s)")),
        "the README says {with_check} against pcbway:\n{said}"
    );
    let (_, said) = run(
        &[
            "score".into(),
            "--preset".into(),
            "pcbway".into(),
            routed.into(),
        ],
        &dir,
    );
    assert!(
        said.contains(&format!("\"drc_violations\": {with_score}")),
        "and `score` has to reach the same figure:\n{said}"
    );
}

#[test]
fn export_refuses_a_shorted_board_unless_forced() {
    // The claim beside the second command in the block, which no file in the
    // repository makes: `examples/blink.cypcb` is not shorted.
    let dir = std::env::temp_dir().join("cypcb-readme-shorted");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("shorted.cypcb");
    std::fs::write(
        &board,
        r#"version 1

board shorted {
    size 30mm x 30mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 5mm, 15mm
}

component R2 resistor "0402" {
    value "10k"
    at 25mm, 15mm
}

component R3 resistor "0402" {
    value "10k"
    at 15mm, 15mm
}

net SIG {
    R1.1
    R2.1
}

trace SIG {
    from R1.1
    to R2.1
    layer Top
    width 0.2mm
}
"#,
    )
    .expect("the fixture is writable");

    let (code, said) = run(
        &[
            "export".into(),
            board.display().to_string(),
            "-o".into(),
            dir.join("out").display().to_string(),
        ],
        &dir,
    );
    assert_eq!(
        code,
        Some(1),
        "the README says export refuses a shorted board:\n{said}"
    );

    let (code, said) = run(
        &[
            "export".into(),
            board.display().to_string(),
            "-o".into(),
            dir.join("forced").display().to_string(),
            "--force".into(),
        ],
        &dir,
    );
    assert_eq!(
        code,
        Some(0),
        "and that `--force` is the way past it:\n{said}"
    );
}
