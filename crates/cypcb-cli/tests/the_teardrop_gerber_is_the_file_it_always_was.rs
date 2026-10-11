//! The copper files a board with teardrops gets are the files it always got.
//!
//! `cargo test -p cypcb-cli --test the_teardrop_gerber_is_the_file_it_always_was`
//!
//! A teardrop became copper in the board and the Gerber writer reads it from
//! there, instead of working the fillet out of the tracks as it wrote. The
//! file must not have moved by a byte: a fabricator who received a board last
//! month and receives it again today is owed the same copper, in the same
//! order. The files under `fixtures/teardrop-gerber/` were written by the
//! command as it stood before the change (commit 97761a3b), with the export
//! time fixed, and this exports the same two boards and holds both copper
//! layers of each against them.
//!
//! - `teardrops.cypcb` asks for teardrops itself: two fillets on the top
//!   layer, which is the smallest board where the order of two regions in the
//!   file can be told.
//! - `usb-diff-pair.cypcb` asks for none and gains four fillets from the
//!   `--teardrops` flag.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/teardrop-gerber")
}

fn scratch(who: &str) -> cypcb_fixtures::ScratchPath {
    let dir_home = cypcb_fixtures::scratch_dir(&format!("cypcb-teardrop-gerber-{who}"));
    let dir = dir_home.join("out");
    dir_home.holding(dir)
}

fn export(board: &str, flag: bool, out: &Path) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cypcb"));
    command.env("SOURCE_DATE_EPOCH", "1760000000");
    command
        .arg("export")
        .arg(fixtures().join(board))
        .arg("-o")
        .arg(out);
    if flag {
        command.arg("--teardrops");
    }
    let status = command.status().expect("the binary runs");
    assert!(status.success(), "the export of {board} failed");
}

fn holds(board: &str, flag: bool, golden: &str, who: &str) {
    let out = scratch(who);
    export(board, flag, &out);
    let name = board.trim_end_matches(".cypcb");
    for layer in ["F_Cu", "B_Cu"] {
        let file = format!("{name}-{layer}.gbr");
        let written = std::fs::read_to_string(out.join("gerber").join(&file))
            .unwrap_or_else(|e| panic!("{file} was not written: {e}"));
        let before = std::fs::read_to_string(fixtures().join(golden).join(&file))
            .unwrap_or_else(|e| panic!("the golden {file} does not read: {e}"));
        assert_eq!(written, before, "{file} is not the file it was");
    }
}

#[test]
fn a_board_that_asks_for_teardrops_gets_the_copper_it_got() {
    holds("teardrops.cypcb", false, "plain", "asks");
}

#[test]
fn a_board_given_teardrops_by_the_flag_gets_the_copper_it_got() {
    holds("usb-diff-pair.cypcb", true, "flag", "flag");
}

#[test]
fn control_the_goldens_carry_the_fillets_they_are_meant_to_hold() {
    // Without this a golden of a board with no fillet would hold nothing.
    let regions = |dir: &str, file: &str| {
        std::fs::read_to_string(fixtures().join(dir).join(file))
            .expect("the golden reads")
            .matches("G36*")
            .count()
    };
    assert_eq!(regions("plain", "teardrops-F_Cu.gbr"), 2);
    assert_eq!(regions("flag", "usb-diff-pair-F_Cu.gbr"), 4);
}
