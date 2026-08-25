//! `-v` turns the tracing calls on, and `RUST_LOG` overrules it.
//!
//! `cargo test -p cypcb-cli --test verbose_says_what_the_router_is_doing`
//!
//! The crates this binary is built from carry tracing calls - which net could
//! not be routed, how many iterations the router took, which variant was
//! skipped and why - and for a long time no command installed a subscriber, so
//! every one of them went nowhere and `RUST_LOG` did nothing either. `-v`
//! exists to fix that, and nothing ran it: counting the flags on every
//! subcommand against the suite, `--verbose` was on all nine and in none of
//! the tests.
//!
//! Measured on `examples/blink.cypcb`: `route` with no flag prints **no** log
//! lines, `route -v` prints **379**, and `RUST_LOG=cypcb_autoroute=debug` with
//! no flag prints **226**. The figures move with the router, so this test
//! holds the shape rather than the count: silence, then INFO, then DEBUG for
//! the one crate that was asked for.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Route the smallest shipped board, and return what the command said.
fn route(who: &str, args: &[&str], rust_log: Option<&str>) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-verbose-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let out = dir.join("routed.cypcb");

    let mut command = Command::new(env!("CARGO_BIN_EXE_cypcb"));
    command
        .arg("route")
        .args(args)
        .arg("examples/blink.cypcb")
        .arg("-o")
        .arg(&out)
        .current_dir(repo_root());
    match rust_log {
        Some(value) => command.env("RUST_LOG", value),
        // Cargo's own environment may carry one, and a test that inherits it
        // is a test measuring the machine it runs on.
        None => command.env_remove("RUST_LOG"),
    };

    let output = command.output().expect("the binary runs");
    assert!(
        output.status.success(),
        "routing the example failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// How many lines look like a tracing line at this level.
fn lines_at(said: &str, level: &str) -> usize {
    said.lines().filter(|line| line.contains(level)).count()
}

#[test]
fn without_the_flag_the_command_says_nothing_about_its_own_work() {
    let said = route("quiet", &[], None);
    assert_eq!(
        lines_at(&said, "INFO"),
        0,
        "a command nobody asked for logs is a command that keeps them to \
         itself:\n{said}"
    );
    assert_eq!(lines_at(&said, "DEBUG"), 0, "{said}");
}

#[test]
fn one_v_turns_the_info_calls_on() {
    let said = route("info", &["-v"], None);
    assert!(
        lines_at(&said, "INFO") > 0,
        "`-v` is the flag that installs the subscriber, and the router has \
         plenty to say:\n{said}"
    );
    assert_eq!(
        lines_at(&said, "DEBUG"),
        0,
        "one `-v` is `info`; `debug` needs a second:\n{said}"
    );
}

#[test]
fn rust_log_is_honoured_without_the_flag_at_all() {
    // The half a person reaches for when they know which crate they want.
    let said = route("env", &[], Some("cypcb_autoroute=debug"));
    assert!(
        lines_at(&said, "DEBUG") > 0,
        "`RUST_LOG` names a crate and a level, and it has to be obeyed on its \
         own:\n{said}"
    );
}
