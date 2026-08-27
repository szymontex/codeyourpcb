//! `route` and `score` both route a board, and not the same way.
//!
//! `cargo test -p cypcb-cli --test the_two_ways_to_route_say_which_they_are`
//!
//! `route` ranks thirteen cost models and keeps the winner; `score` runs the
//! shipped defaults once; `route --fast` runs the defaults and says so. All
//! three are defensible and the pages said none of it, so two numbers taken
//! from the same board by two commands looked comparable and were not.
//!
//! This holds each page to what its command does.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn run(args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn each_page_says_which_routing_it_does() {
    let route = run(&["route", "--help"]);
    assert!(
        route.contains("Thirteen variants"),
        "the search is what makes a run take as long as it does:\n{route}"
    );
    assert!(
        route.contains("`--fast`"),
        "and the flag that skips it:\n{route}"
    );

    let score = run(&["score", "--help"]);
    assert!(
        score.contains("no variant search"),
        "scoring measures a board rather than searching for one:\n{score}"
    );
    assert!(
        score.contains("route --fast"),
        "and names the run that matches it:\n{score}"
    );
}

#[test]
fn the_commands_do_what_their_pages_say() {
    let dir = std::env::temp_dir().join("cypcb-two-ways-to-route");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let routed = dir.join("routed.cypcb");
    let searched = run(&[
        "route",
        "examples/routing-test.cypcb",
        "-o",
        routed.to_str().expect("a path"),
    ]);
    assert!(
        searched.contains("13 variants"),
        "the search the page describes:\n{searched}"
    );
    assert!(
        searched.contains("Chose "),
        "and the winner it names:\n{searched}"
    );

    // The defaults, once: no ranking, no winner named.
    let fast = dir.join("fast.cypcb");
    let quick = run(&[
        "route",
        "--fast",
        "examples/routing-test.cypcb",
        "-o",
        fast.to_str().expect("a path"),
    ]);
    assert!(
        !quick.contains("13 variants"),
        "`--fast` skips the search:\n{quick}"
    );

    // And scoring a board that already carries copper measures it as it
    // stands, which is the sentence the page ends on.
    let scored = run(&["score", routed.to_str().expect("a path")]);
    assert!(
        scored.contains("trace(s) the file carries"),
        "a routed board is measured rather than routed again:\n{scored}"
    );
}
