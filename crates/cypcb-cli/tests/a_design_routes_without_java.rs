//! `cypcb route board.cypcb` routes the board.
//!
//! `cargo test -p cypcb-cli --test a_design_routes_without_java`
//!
//! It did not. The default sat on FreeRouting - a Java program this binary
//! cannot supply - so the plain command failed on any machine without a jar,
//! with the built-in router compiled in and one flag away. The reason was
//! recorded in the flag's own doc comment: which router this project bets on
//! was an open decision. **D1 closed on 2026-08-09 in favour of the in-house
//! router**, and nothing went back for the default.
//!
//! FreeRouting is still reachable. It is opt-in now, by naming its jar, which
//! is what a Java program needs anyway.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

/// A copy of an example, so the routed output lands in the scratch directory.
fn scratch_copy(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-route-default-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples/blink.cypcb");
    let target = dir.join("blink.cypcb");
    std::fs::copy(&source, &target).expect("the example is copyable");
    target
}

#[test]
fn the_plain_command_routes_the_board() {
    let board = scratch_copy("plain");
    let output = cypcb()
        .arg("route")
        .arg(&board)
        .env_remove("RUST_LOG")
        .env_remove("FREEROUTING_JAR")
        .output()
        .expect("the binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        output.status.success(),
        "`cypcb route <file>` has to route a board on a machine with no Java:\n{stderr}"
    );
    assert!(
        !stderr.contains("FreeRouting"),
        "nothing about the default run goes near a jar:\n{stderr}"
    );
    assert!(
        stderr.contains("Wrote "),
        "and it says what it wrote:\n{stderr}"
    );

    let routed = board.with_extension("routed.cypcb");
    let written = std::fs::read_to_string(&routed).expect("the routed design is on disk");
    assert!(
        written.contains("trace "),
        "a routed design carries trace blocks:\n{written}"
    );
}

#[test]
fn naming_a_jar_still_asks_for_freerouting() {
    // The other half: opt-in has to actually opt in, or this commit deleted a
    // feature instead of moving a default. The jar does not exist, so the run
    // fails - what it fails saying is the point.
    let board = scratch_copy("jar");
    let output = cypcb()
        .arg("route")
        .arg(&board)
        .arg("--freerouting")
        .arg("/nonexistent/freerouting.jar")
        .env_remove("RUST_LOG")
        .output()
        .expect("the binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        !output.status.success(),
        "a jar that is not there cannot route anything:\n{stderr}"
    );
    assert!(
        stderr.contains("FreeRouting JAR not found"),
        "the failure has to be the jar that was named, not a refused flag:\n{stderr}"
    );
    assert!(
        !stderr.contains("Drop the flag"),
        "routing in-house and then refusing the flag is not honouring it:\n{stderr}"
    );
    assert!(
        !board.with_extension("routed.cypcb").exists(),
        "and it must not quietly route in-house instead"
    );
}
