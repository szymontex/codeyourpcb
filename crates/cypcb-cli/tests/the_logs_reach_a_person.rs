//! What the crates write down, a user can read.
//!
//! `cargo test -p cypcb-cli --test the_logs_reach_a_person`
//!
//! `cypcb-autoroute` alone carries 76 `tracing` calls - how many iterations
//! the negotiated-congestion loop took, whether it converged, which nets it
//! gave up on, which variant failed and was dropped from the ranking - and no
//! command installed a subscriber. Every one of them went nowhere, and
//! `RUST_LOG` did nothing, so a user whose board came back half-routed had no
//! way to ask why.
//!
//! Warnings are on by default now and go to stderr. Louder levels are asked
//! for with `-v`, or named exactly through `RUST_LOG`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn scratch_copy(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-logs-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark/led_blink.kicad_pcb");
    let target = dir.join("led_blink.kicad_pcb");
    std::fs::copy(&source, &target).expect("the fixture is copyable");
    target
}

/// Route a board, returning stdout and stderr separately.
fn route(board: &Path, extra: &[&str], env: Option<(&str, &str)>) -> (String, String) {
    let mut command = cypcb();
    command.arg("route").arg(board).arg("--fast").args(extra);
    match env {
        Some((key, value)) => {
            command.env(key, value);
        }
        // Inherited RUST_LOG would decide this test's outcome instead of the
        // flag under test.
        None => {
            command.env_remove("RUST_LOG");
        }
    }
    let output = command.output().expect("the binary runs");
    assert!(
        output.status.success(),
        "routing failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn verbose_shows_what_the_router_did() {
    let board = scratch_copy("verbose");
    let (_, stderr) = route(&board, &["-v"], None);

    assert!(
        stderr.contains("All nets routed successfully"),
        "the router says whether it finished, and -v has to show it:\n{stderr}"
    );
    assert!(
        stderr.contains("iterations"),
        "and how many iterations that took:\n{stderr}"
    );
}

#[test]
fn the_default_run_is_as_quiet_as_it_was() {
    // The half that must not change. A log line in the middle of a command's
    // own output is a command nobody can pipe.
    let board = scratch_copy("quiet");
    let (_, stderr) = route(&board, &[], None);

    assert!(
        !stderr.contains("INFO"),
        "info belongs behind -v, not in every run:\n{stderr}"
    );
    assert!(
        stderr.contains("Wrote "),
        "the command's own output is still there:\n{stderr}"
    );
}

#[test]
fn rust_log_names_one_crate() {
    // The reason a subscriber beats a boolean: a reader who knows the syntax
    // can ask for one crate and get nothing from the rest.
    let board = scratch_copy("env");
    let (_, stderr) = route(&board, &[], Some(("RUST_LOG", "cypcb_autoroute=info")));

    assert!(
        stderr.contains("All nets routed successfully"),
        "RUST_LOG has to reach the router:\n{stderr}"
    );
}

#[test]
fn nothing_the_logger_writes_lands_on_stdout() {
    // `score` and `parse` print JSON on stdout and a log line in the middle of
    // it makes the output unparseable. Proven on the loudest setting there is.
    let board = scratch_copy("stdout");
    let output = cypcb()
        .arg("score")
        .arg(&board)
        .arg("-vvv")
        .env_remove("RUST_LOG")
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "scoring failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_str::<serde_json::Value>(&stdout)
        .unwrap_or_else(|e| panic!("stdout has to stay pure JSON ({e}):\n{stdout}"));
}

#[test]
fn the_ranked_line_says_how_many_contacts_the_violations_describe() {
    // A variant list ranked on violation counts is a list ranked on rows, and
    // the clearance rule reports per pair of segments - so two of the lines
    // below can differ by a violation and describe the same contact. Decided
    // 2026-08-23: the counts stay as they are and the contact count is printed
    // beside them, which is what makes the difference readable rather than
    // confusing.
    //
    // Not through `route` above: that helper always passes `--fast`, and fast
    // mode scores one candidate and prints no ranked list at all.
    let board = scratch_copy("ranked-contacts");
    let output = cypcb()
        .arg("route")
        .arg(&board)
        .env_remove("RUST_LOG")
        .output()
        .expect("the binary runs");
    assert!(output.status.success(), "routing failed");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let ranked: Vec<&str> = stderr
        .lines()
        .filter(|line| line.contains("composite"))
        .collect();
    assert!(
        !ranked.is_empty(),
        "a default run ranks its candidates and says so:\n{stderr}"
    );
    for line in &ranked {
        assert!(
            line.contains("clearance contacts"),
            "every ranked line names the contacts its violations describe: {line}"
        );
    }
}
