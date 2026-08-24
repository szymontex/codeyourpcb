//! A routed design says how it was routed.
//!
//! `cargo test -p cypcb-cli --test the_routed_file_says_how_it_was_routed`
//!
//! The file carried one line about its own copper: *"Traces below were
//! produced by `cypcb route --in-house`."* Both halves had gone wrong. The
//! flag stopped being needed when the built-in router became the default, so
//! the line named a command nobody has to type; and a default run ranks
//! thirteen variants and keeps one, which the file never said - so a board
//! could not be reproduced from the file it produced, on a project where
//! `router-is-repeatable` is a gate stage.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

/// A copy of an example, so the routed output lands in the scratch directory.
fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-routed-header-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples/blink.cypcb");
    let board = dir.join("blink.cypcb");
    std::fs::copy(&source, &board).expect("the example is copyable");
    board
}

/// Route it, returning what it said and what it wrote.
fn route(board: &Path, extra: &[&str]) -> (String, String) {
    let output = cypcb()
        .arg("route")
        .arg(board)
        .args(extra)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "routing failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let routed = board.with_extension("routed.cypcb");
    (
        String::from_utf8_lossy(&output.stderr).to_string(),
        std::fs::read_to_string(&routed).expect("the routed design is on disk"),
    )
}

#[test]
fn a_best_of_run_names_the_variant_it_kept() {
    let board = scratch("variants");
    let (said, written) = route(&board, &[]);

    // Whatever won on the day, the file has to name the same one the run did.
    let chosen = said
        .lines()
        .find_map(|line| line.strip_prefix("Chose "))
        .expect("the run says which variant it chose")
        .trim()
        .to_string();

    assert!(
        written.contains(&format!("`{chosen}`")),
        "the run chose `{chosen}` and the file does not say so:\n{}",
        written
            .lines()
            .find(|l| l.contains("produced by"))
            .unwrap_or("")
    );
    assert!(
        written.contains("best of 13 variants"),
        "and how many it was chosen from:\n{written}"
    );
}

#[test]
fn a_fast_run_says_it_took_the_default_settings() {
    // The other branch. It routes once and keeps that, so naming a variant
    // would be a claim about a ranking that never happened.
    let board = scratch("fast");
    let (_, written) = route(&board, &["--fast"]);

    let header = written
        .lines()
        .find(|line| line.contains("produced by"))
        .expect("the file says how it was routed");
    assert!(header.contains("--fast"), "{header}");
    assert!(header.contains("default settings"), "{header}");
    assert!(
        !header.contains("best of"),
        "nothing was ranked, so nothing was best of anything: {header}"
    );
}

#[test]
fn neither_run_tells_a_reader_to_type_a_flag_that_is_the_default() {
    // `--in-house` still works and is still documented; it stopped being the
    // way to reach the built-in router when that became the default, and a
    // file telling a person to type it is advice that was true once.
    for extra in [&[][..], &["--fast"][..]] {
        let board = scratch(if extra.is_empty() {
            "flag-default"
        } else {
            "flag-fast"
        });
        let (_, written) = route(&board, extra);
        assert!(
            !written.contains("--in-house"),
            "the routed file still names `--in-house`:\n{written}"
        );
    }
}
