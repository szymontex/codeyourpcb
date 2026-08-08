//! The output directory is the unit somebody zips and sends.
//!
//! `cargo test -p cypcb-cli --test an_export_directory_is_what_gets_sent`
//!
//! Exporting a second board into a directory that still holds the first leaves
//! both, silently. Measured with `four-layer` then `blink` into one directory:
//! twenty Gerbers for two different boards, including `In1_Cu` and `In2_Cu`
//! copper that belongs to neither the two-layer board being sent nor anything
//! the fabricator was asked for. A CAM operator opening that zip sees a
//! four-layer stack and a two-layer stack and has to guess.
//!
//! Overwriting the same board's own files is ordinary - it is what re-exporting
//! after a change does - and stays silent. Copper from a different board does
//! not.

use std::path::PathBuf;
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn example(name: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
        .join(name)
}

fn export_into(board: &str, dir: &PathBuf) -> String {
    let output = cypcb()
        .arg("export")
        .arg(example(board))
        .arg("-o")
        .arg(dir)
        .arg("--preset")
        .arg("jlcpcb")
        .output()
        .expect("the binary runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-export-dir-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_second_board_in_the_same_directory_is_reported() {
    let dir = scratch("two-boards");
    export_into("four-layer.cypcb", &dir);
    let out = export_into("blink.cypcb", &dir);

    assert!(
        out.contains("were not written by this export"),
        "the first board's files travel with the second and nothing said so:\n{out}"
    );
    // Named, so the reader can delete them.
    assert!(
        out.contains("four-layer-"),
        "the warning has to name the files:\n{out}"
    );
    // The inner-layer copper is the part that matters: it belongs to a stack
    // the board being sent does not have.
    assert!(
        out.contains("In1_Cu") || out.contains("and") && out.contains("more"),
        "the four-layer board's inner copper is in the directory:\n{out}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn re_exporting_the_same_board_says_nothing() {
    // The half that must not change. Exporting again after a change is the
    // ordinary way to use this command, and its own files are supposed to be
    // overwritten.
    let dir = scratch("same-board");
    export_into("blink.cypcb", &dir);
    let out = export_into("blink.cypcb", &dir);

    assert!(
        !out.contains("were not written by this export"),
        "re-exporting the same board warned about its own files:\n{out}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_fresh_directory_says_nothing() {
    let dir = scratch("fresh");
    let out = export_into("blink.cypcb", &dir);
    assert!(
        !out.contains("were not written by this export"),
        "an empty directory had strangers in it:\n{out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
