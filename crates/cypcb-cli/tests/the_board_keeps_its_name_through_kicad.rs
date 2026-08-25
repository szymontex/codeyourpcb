//! A board keeps its name through KiCad.
//!
//! `cargo test -p cypcb-cli --test the_board_keeps_its_name_through_kicad`
//!
//! Every design that went out to KiCad and came back was called `KiCad PCB`,
//! whatever its author had called it, and nothing said so. The round-trip
//! census found it: with the outline fixed the diff between a design and
//! itself after a trip is the list of what the pair loses, and the first line
//! of that list on every example was the board's own name.
//!
//! KiCad keeps no field for a board name on the board. What it has is the
//! title block - where pcbnew puts the name a person reads - so that is where
//! this project writes it, and where it reads it back. A board pcbnew wrote
//! usually states nothing there, and those still arrive under the name this
//! importer has always given them.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn cypcb(args: &[&str]) {
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
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-board-name-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    dir
}

#[test]
fn the_name_is_written_into_the_title_block_and_read_back() {
    let dir = scratch("trip");
    let kicad = dir.join("blink.kicad_pcb");
    let back = dir.join("blink.cypcb");

    cypcb(&[
        "to-kicad",
        "examples/blink.cypcb",
        "-o",
        kicad.to_str().expect("a path that is text"),
    ]);
    let written = std::fs::read_to_string(&kicad).expect("the KiCad board was written");
    assert!(
        written.contains("(title_block") && written.contains("(title \"blink\")"),
        "the design is called `blink` and the title block is where that goes:\n{}",
        written.lines().take(12).collect::<Vec<_>>().join("\n")
    );

    cypcb(&[
        "from-kicad",
        kicad.to_str().expect("a path that is text"),
        "-o",
        back.to_str().expect("a path that is text"),
    ]);
    let source = std::fs::read_to_string(&back).expect("the design came back");
    assert!(
        source.contains("board blink {"),
        "and it has to come home under its own name:\n{source}"
    );
}

#[test]
fn a_board_that_names_itself_nowhere_keeps_the_name_it_always_had() {
    // The half that keeps the other from being a rename: `led_blink` is a
    // board pcbnew wrote, with no title in it, and it has always imported as
    // `KiCad PCB`.
    let dir = scratch("untitled");
    let back = dir.join("led_blink.cypcb");

    cypcb(&[
        "from-kicad",
        "tests/fixtures/benchmark/led_blink.kicad_pcb",
        "-o",
        back.to_str().expect("a path that is text"),
    ]);
    let source = std::fs::read_to_string(&back).expect("the design came back");
    assert!(
        source.contains("board KiCad_PCB {"),
        "a board with no title of its own keeps the importer's name for it:\n{source}"
    );
}
