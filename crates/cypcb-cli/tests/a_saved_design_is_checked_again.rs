//! `cypcb watch` re-checks a design when it is saved.
//!
//! `cargo test -p cypcb-cli --test a_saved_design_is_checked_again`
//!
//! The browser has had hot reload since the dev server was written; a terminal
//! had nothing, so checking a board meant running `cypcb check` by hand after
//! every save. `cypcb-watcher` was written for exactly this and had no caller
//! anywhere - 184 lines and three passing tests that nothing in the workspace
//! used, the last crate in that state.
//!
//! Timing-tolerant on purpose: it waits for output to appear rather than
//! sleeping a fixed amount, because a test that fails when the machine is busy
//! teaches people to ignore it.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const BOARD: &str = r#"version 1

board watched {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 10mm
}
"#;

/// How long to wait for the watcher to notice something.
///
/// Sized for a machine under load rather than an idle one. At 20s this test
/// passed on its own in 0.41s and failed inside the full gate at 20.02s -
/// exactly the deadline, with the rest of the workspace compiling and testing
/// alongside it. The docstring above says a test that fails when the machine
/// is busy teaches people to ignore it, and then it did that.
///
/// A generous deadline costs nothing when the watcher works: the loop returns
/// as soon as the log says what it is waiting for, so the only run that takes
/// a minute is a run that was going to fail anyway.
const PATIENCE: Duration = Duration::from_secs(60);

/// Read the log until it holds `count` checks, or give up.
fn wait_for_checks(log: &std::path::Path, count: usize, what: &str) -> String {
    let started = Instant::now();
    let deadline = started + PATIENCE;
    loop {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        if text.matches("DRC violation").count() + text.matches("passed DRC").count() >= count {
            return text;
        }
        if Instant::now() > deadline {
            panic!(
                "waited {:.1}s for {what}; the log holds:\n{text}",
                started.elapsed().as_secs_f64()
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn saving_a_design_checks_it_again() {
    let dir = std::env::temp_dir().join("cypcb-watch-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, BOARD).expect("the board is written");
    let log_path = dir.join("watch.log");
    let log = std::fs::File::create(&log_path).expect("a log to read");

    let mut child = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["watch"])
        .arg(&board)
        .stdout(Stdio::from(log.try_clone().expect("the log is shareable")))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("the binary runs");

    // The first check happens before anything is watched.
    let first = wait_for_checks(&log_path, 1, "the first check");
    assert!(
        first.contains("Watching"),
        "it says what it is watching:\n{first}"
    );

    // A second part, which brings two more unconnected pins with it.
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&board)
        .expect("the board is writable");
    writeln!(
        file,
        "\ncomponent R2 resistor \"0402\" {{\n    value \"1k\"\n    at 20mm, 10mm\n}}"
    )
    .expect("the edit is written");
    drop(file);

    let after = wait_for_checks(&log_path, 2, "a check after the save");
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        after.contains("changed"),
        "it says which file it re-read:\n{after}"
    );

    // One save, one check. The debouncer forwards a save as a stream of
    // notifications - measured at one every 200ms for as long as the command
    // ran - and without the content check that was 24 re-checks for one edit.
    let checks = after.matches("DRC violation").count() + after.matches("passed DRC").count();
    assert!(
        checks <= 3,
        "one edit, {checks} checks - the event stream is not being coalesced:\n{after}"
    );

    // And the second check saw the new part: R1 alone is two unconnected pins,
    // R1 and R2 are four.
    let counts: Vec<&str> = after
        .lines()
        .filter(|line| line.contains("DRC violation"))
        .collect();
    assert!(
        counts.len() >= 2 && counts[0] != counts[1],
        "the re-check read the file again rather than repeating itself:\n{after}"
    );
}
