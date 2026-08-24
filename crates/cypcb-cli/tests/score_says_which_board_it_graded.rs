//! `cypcb score` says which router produced the board it is grading.
//!
//! `cargo test -p cypcb-cli --test score_says_which_board_it_graded`
//!
//! `score` routes a file that carries no copper, and it routes it **once with
//! the default settings**. `cypcb route` on the same file ranks thirteen
//! variants and keeps the best, which is a different board - measured on
//! `examples/blink.cypcb` in this project's own notes: one run gives 9
//! violations with 6 shorts, best-of gives 5 with 3.
//!
//! So the two commands grade different copper from the same source and the
//! JSON says nothing about which. A reader comparing the numbers should not
//! have to read `score.rs` to find that out.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
        .join(name)
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-score-says-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    dir
}

/// Score it, returning stderr and stdout.
fn score(board: &Path) -> (String, String) {
    let output = cypcb()
        .arg("score")
        .arg(board)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "scoring failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        String::from_utf8_lossy(&output.stderr).to_string(),
        String::from_utf8_lossy(&output.stdout).to_string(),
    )
}

#[test]
fn an_unrouted_board_is_told_which_router_laid_the_copper() {
    let (said, json) = score(&example("blink.cypcb"));

    assert!(
        said.contains("once with the default settings"),
        "scoring routes the board itself, and how it routed decides every number below:\n{said}"
    );
    assert!(
        said.contains("13 variants"),
        "and the other command produces a different board, which is worth knowing:\n{said}"
    );
    assert!(
        json.contains("\"drc_violations\""),
        "the metrics still land on stdout, unmixed:\n{json}"
    );
}

#[test]
fn a_board_that_carries_copper_is_not_routed_again() {
    // The half this message must not break. A file with traces is scored as
    // it stands - routing over it once laid a second routing on top of the
    // first and measured the pile, which is why this branch exists.
    let dir = scratch("routed");
    let board = dir.join("blink.cypcb");
    std::fs::copy(example("blink.cypcb"), &board).expect("the example is copyable");

    let routed = cypcb()
        .arg("route")
        .arg(&board)
        .arg("--fast")
        .output()
        .expect("the binary runs");
    assert!(routed.status.success(), "routing failed");

    let (said, _) = score(&board.with_extension("routed.cypcb"));
    assert!(
        said.contains("trace(s) the file carries"),
        "a board with copper is graded as it stands:\n{said}"
    );
    assert!(
        !said.contains("routing it once"),
        "and nothing is laid on top of it:\n{said}"
    );
}
