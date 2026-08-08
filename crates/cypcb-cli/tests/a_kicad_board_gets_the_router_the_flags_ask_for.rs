//! The routing flags mean the same thing whatever kind of file is routed.
//!
//! `cargo test -p cypcb-cli --test a_kicad_board_gets_the_router_the_flags_ask_for`
//!
//! `cypcb route` has two branches: a `.cypcb` board and a `.kicad_pcb` board.
//! The first routes eight ways and keeps the best unless `--fast` is given.
//! The second called the router once with the default settings whatever the
//! flags said, so `--variants` and `--fast` were accepted and ignored on every
//! KiCad board.
//!
//! It was not a small difference. `multi_ic`, release build:
//!
//!   one default run   291 DRC violations, 187 shorts, 119 vias,  5.88s
//!   best of eight     165 DRC violations,  86 shorts, 167 vias, 86.03s
//!
//! The winner was `PathFinder Pad Aware`; `PathFinder Default` - the setting
//! every KiCad board used to get - came sixth of the eight. A user routing a
//! KiCad board was handed nearly twice the shorts and had no flag that could
//! change it.

use std::path::PathBuf;
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A copy of the smallest benchmark board, in a directory of this test's own.
fn scratch_copy(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-route-flags-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let target = dir.join("led_blink.kicad_pcb");
    std::fs::copy(
        repo_root().join("tests/fixtures/benchmark/led_blink.kicad_pcb"),
        &target,
    )
    .expect("the fixture is copyable");
    target
}

fn route(board: &PathBuf, flags: &[&str]) -> String {
    let output = cypcb()
        .arg("route")
        .arg(board)
        .args(flags)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "routing failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn a_kicad_board_is_routed_several_ways_and_the_best_is_kept() {
    let board = scratch_copy("default");
    let out = route(&board, &[]);

    assert!(
        out.contains("variants and keeping the best"),
        "a KiCad board gets the same best-of-N a .cypcb board gets:\n{out}"
    );
    assert!(
        out.contains("Chose "),
        "and it has to say which setting it kept:\n{out}"
    );
    assert!(
        board.with_extension("routed.kicad_pcb").exists(),
        "the routed board still has to be written"
    );
}

#[test]
fn fast_means_one_run_on_a_kicad_board_too() {
    let board = scratch_copy("fast");
    let out = route(&board, &["--fast"]);

    assert!(
        !out.contains("variants and keeping the best"),
        "--fast asks for one run and has to get it:\n{out}"
    );
    assert!(
        board.with_extension("routed.kicad_pcb").exists(),
        "the routed board still has to be written"
    );
}

#[test]
fn the_two_ways_do_not_produce_the_same_file_by_accident() {
    // The assertions above read messages. This one reads the copper: if the
    // flag changed nothing, both boards would be identical, which is exactly
    // what the defect looked like - `--variants` and `--fast` produced byte
    // for byte the same 945 segments and 119 vias on `multi_ic`.
    //
    // `led_blink` is small enough that both ways may well find the same
    // routing, so this asserts the weaker thing that still bites: each way
    // writes a board with copper in it, and the run without `--fast` scored
    // more than one candidate.
    let default_board = scratch_copy("compare-default");
    let default_out = route(&default_board, &[]);
    let fast_board = scratch_copy("compare-fast");
    route(&fast_board, &["--fast"]);

    let ranked = default_out
        .lines()
        .filter(|line| {
            line.trim_start().starts_with(char::is_numeric) && line.contains("composite")
        })
        .count();
    assert!(
        ranked >= 2,
        "best-of-N has to score more than one candidate and say so:\n{default_out}"
    );

    for board in [&default_board, &fast_board] {
        let routed = std::fs::read_to_string(board.with_extension("routed.kicad_pcb"))
            .expect("a routed board is readable");
        assert!(
            routed
                .lines()
                .any(|line| line.trim_start().starts_with("(segment")),
            "{} came back with no copper on it",
            board.display()
        );
    }
}
