//! `route --fast` writes the same board every time it is run.
//!
//! `cargo test -p cypcb-cli --test a_route_says_the_same_each_time`
//!
//! Every number measured about the router is one run of this command, and
//! every comparison between two of them assumes a second run would have
//! written the same file. Rust seeds each hash map afresh per process, and
//! bevy finds a query's tables through one, so anything that orders work by
//! walking a map routes a different board next time - and one process cannot
//! show it, because the seed is fixed for its life. This routes each
//! benchmark board in two processes of the binary itself and asks for the
//! same bytes and the same DRC line from both.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The six KiCad benchmark boards and the one written in the language.
const BOARDS: [&str; 7] = [
    "led_blink.kicad_pcb",
    "multi_ic.kicad_pcb",
    "plane_board.kicad_pcb",
    "qfp_fanout.kicad_pcb",
    "shift_driver.kicad_pcb",
    "stm32_breakout.kicad_pcb",
    "esp32_starter.cypcb",
];

const RUNS: usize = 2;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/benchmark")
        .join(name)
}

/// The file one run wrote and the line it printed about the routed board.
fn route_once(board: &str, run: usize, dir: &Path) -> (Vec<u8>, String) {
    let (stem, extension) = board.split_once('.').expect("a fixture has an extension");
    let written = dir.join(format!("{stem}-{run}.{extension}"));
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("route")
        .arg("--fast")
        .arg(fixture(board))
        .arg("-o")
        .arg(&written)
        .output()
        .expect("the binary starts");
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "routing {board} failed:\n{printed}"
    );
    let drc = printed
        .lines()
        .find(|line| line.starts_with("DRC on the routed board"))
        .unwrap_or_else(|| panic!("routing {board} printed no DRC line:\n{printed}"))
        .to_string();
    let bytes = std::fs::read(&written)
        .unwrap_or_else(|e| panic!("routing {board} wrote no {}: {e}", written.display()));
    (bytes, drc)
}

#[test]
fn every_benchmark_board_routes_to_the_same_file() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("a_route_says_the_same_each_time");
    std::fs::create_dir_all(&dir).expect("a scratch directory");

    let mut moved = Vec::new();
    for board in BOARDS {
        let runs: Vec<(Vec<u8>, String)> =
            (1..=RUNS).map(|run| route_once(board, run, &dir)).collect();
        let (first_file, first_drc) = &runs[0];
        for (index, (file, drc)) in runs.iter().enumerate().skip(1) {
            if file != first_file {
                moved.push(format!("{board}: run {} wrote a different file", index + 1));
            }
            if drc != first_drc {
                moved.push(format!("{board}: `{first_drc}` then `{drc}`"));
            }
        }
    }

    assert!(
        moved.is_empty(),
        "the same board routed in another process has to be the same board:\n{}",
        moved.join("\n")
    );
}
