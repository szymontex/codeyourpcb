//! Route a KiCad board and get a KiCad board back.
//!
//! `cargo test -p cypcb-cli --test a_kicad_board_comes_back_routed`
//!
//! The loop a KiCad user has is: draw the board in KiCad, route it, open it in
//! KiCad. This project could do the middle step and neither end of it. `check`
//! and `export` learned to read a board one commit ago; `route` still handed
//! the file to the DSL parser, and the path it takes for `.cypcb` - append
//! trace blocks to a copy of the source - would have produced a
//! `(kicad_pcb ...)` file with DSL stuck on the end, which neither reader can
//! open.
//!
//! What is written is narrow on purpose: the `(segment ...)` and `(via ...)`
//! forms routing produces, inserted into a copy of the original. Everything
//! this project models loosely or not at all - footprints, setup, zones it
//! would not approximate - is carried through byte for byte, because the
//! safest way to preserve what you do not model is not to rewrite it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A copy of a fixture, so the routed output lands beside it in a scratch dir.
fn scratch_copy(fixture: &str, who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-route-kicad-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let source = repo_root().join("tests/fixtures/benchmark").join(fixture);
    let target = dir.join(fixture);
    std::fs::copy(&source, &target).expect("the fixture is copyable");
    target
}

fn check(path: &Path) -> String {
    let output = cypcb()
        .arg("check")
        .arg(path)
        .output()
        .expect("the binary runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn routing_a_kicad_board_writes_a_kicad_board() {
    let board = scratch_copy("plane_board.kicad_pcb", "writes");
    let before = std::fs::read_to_string(&board).expect("readable fixture");

    let output = cypcb()
        .arg("route")
        .arg(&board)
        // `--fast`: these tests are about a KiCad board coming back a KiCad
        // board, not about which routing setting wins. Best-of-eight is the
        // default and costs eight runs.
        .arg("--fast")
        .output()
        .expect("the binary runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "routing a KiCad board failed:\n{combined}"
    );

    let routed_path = board.with_extension("routed.kicad_pcb");
    let routed = std::fs::read_to_string(&routed_path)
        .unwrap_or_else(|e| panic!("{} is readable: {e}\n{combined}", routed_path.display()));

    let segments = routed
        .lines()
        .filter(|line| line.trim_start().starts_with("(segment"))
        .count();
    let vias = routed
        .lines()
        .filter(|line| line.trim_start().starts_with("(via"))
        .count();
    assert!(
        segments > 100,
        "this board takes about 210 segments to route; got {segments}\n{combined}"
    );
    assert!(vias > 0, "its ground pads need stitching vias; got {vias}");

    // Everything the file already said is still there, untouched. The original
    // is a prefix of the result up to its own closing paren.
    let head = before.rfind(')').expect("the fixture is a board");
    assert!(
        routed.starts_with(&before[..head]),
        "the original board was rewritten rather than added to"
    );
    assert!(
        routed.trim_end().ends_with(')'),
        "the result has to still close its own s-expression"
    );

    let _ = std::fs::remove_dir_all(routed_path.parent().expect("a scratch dir"));
}

#[test]
fn the_board_that_comes_back_reads_as_routed() {
    // The round trip is the point. Written coordinates that look plausible but
    // land in the wrong place would pass every check above and connect
    // nothing - which is exactly the defect the importer carried in the other
    // direction, reading copper without subtracting the board origin.
    let board = scratch_copy("plane_board.kicad_pcb", "roundtrip");

    let before = check(&board);
    let unrouted_before = before
        .lines()
        .filter(|line| line.contains("unrouted-pin at"))
        .count();
    assert!(
        unrouted_before > 20,
        "the fixture starts unrouted; got {unrouted_before}\n{before}"
    );

    let output = cypcb()
        .arg("route")
        .arg(&board)
        // `--fast`: these tests are about a KiCad board coming back a KiCad
        // board, not about which routing setting wins. Best-of-eight is the
        // default and costs eight runs.
        .arg("--fast")
        .output()
        .expect("the binary runs");
    assert!(output.status.success());

    let routed_path = board.with_extension("routed.kicad_pcb");
    let after = check(&routed_path);
    let unrouted_after = after
        .lines()
        .filter(|line| line.contains("unrouted-pin at"))
        .count();

    assert_eq!(
        unrouted_after, 0,
        "the routed board still has unreached pins, so the copper written back \
         is not where the pads are: {unrouted_before} before, {unrouted_after} \
         after\n{after}"
    );

    let _ = std::fs::remove_dir_all(routed_path.parent().expect("a scratch dir"));
}
