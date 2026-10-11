//! A board without teardrops is written by every writer as it always was.
//!
//! `cargo test -p cypcb-cli --test the_writers_leave_a_board_without_teardrops_alone`
//!
//! The SVG, DXF, PDF and IPC-2581 writers and `to-kicad` gained the teardrop
//! copper, and each is meant to change only for a board that holds some. The
//! files under `fixtures/teardrop-writers/` were written by the commands as
//! they stood before the change, with the export time fixed, for two boards
//! that ask for none: `blink` and `pour-island` (which carries a pour, so the
//! IPC-2581 and plot paths that read a zone are in it). The same commands are
//! run here and every byte is held against them.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/teardrop-writers")
}

fn scratch(who: &str) -> cypcb_fixtures::ScratchPath {
    let home = cypcb_fixtures::scratch_dir(&format!("cypcb-teardrop-writers-{who}"));
    let dir = home.join("out");
    std::fs::create_dir_all(&dir).expect("the directory is creatable");
    home.holding(dir)
}

fn same(written: &Path, golden: &Path) {
    let got = std::fs::read(written)
        .unwrap_or_else(|e| panic!("{} is not written: {e}", written.display()));
    let want =
        std::fs::read(golden).unwrap_or_else(|e| panic!("{} does not read: {e}", golden.display()));
    assert!(
        got == want,
        "{} is not the file it was ({} bytes against {})",
        golden.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
        got.len(),
        want.len()
    );
}

fn holds(board: &str) {
    let name = board.trim_end_matches(".cypcb");
    let golden = fixtures().join(name);
    let out = scratch(&format!("{name}-export"));
    let status = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .env("SOURCE_DATE_EPOCH", "1760000000")
        .arg("export")
        .arg(fixtures().join(board))
        .arg("-o")
        .arg(&out)
        .args(["--svg", "--dxf", "--pdf", "--ipc2581"])
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export of {board} failed");
    for ext in ["svg", "dxf", "pdf"] {
        let file = format!("{name}-F_Cu.{ext}");
        same(&out.join("plot").join(&file), &golden.join(&file));
    }
    same(
        &out.join("handoff").join(format!("{name}.xml")),
        &golden.join(format!("{name}.xml")),
    );

    let kicad_home = scratch(&format!("{name}-kicad"));
    let kicad = kicad_home.join(format!("{name}.kicad_pcb"));
    let status = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("to-kicad")
        .arg(fixtures().join(board))
        .arg("-o")
        .arg(&kicad)
        .status()
        .expect("the binary runs");
    assert!(status.success(), "to-kicad of {board} failed");
    same(&kicad, &golden.join(format!("{name}.kicad_pcb")));
}

#[test]
fn blink_is_written_as_it_always_was() {
    holds("blink.cypcb");
}

#[test]
fn a_board_with_a_pour_is_written_as_it_always_was() {
    holds("pour-island.cypcb");
}

#[test]
fn control_these_boards_ask_for_none() {
    // Without this a golden of a board that does ask would pass for the wrong
    // reason: the files would hold fillets and the comparison would still hold.
    for board in ["blink.cypcb", "pour-island.cypcb"] {
        let source = std::fs::read_to_string(fixtures().join(board)).expect("the board reads");
        assert!(!source.contains("teardrops"), "{board} asks for teardrops");
    }
}
