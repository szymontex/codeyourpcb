//! A USB board survives the trip from KiCad.
//!
//! `cargo test -p cypcb-cli --test a_usb_board_comes_back_with_its_names`
//!
//! Two names on every USB design there is: a receptacle's pads are called
//! **A1, A4, B4, S1** rather than 1, 2, 3, and its power net is **`VBUS+`**
//! with a differential pair called `D-` beside it. Neither is an identifier,
//! and `from-kicad` used to refuse such a board by name, saying the language
//! could not state the pads.
//!
//! It can. `pad_definition` takes a name or a quoted name, `net_name` takes an
//! identifier or a quoted string, and `dsl.rs` decides which form each name
//! needs on the way out - `pad A1` bare, `net "VBUS+"` quoted, because a `+`
//! cannot be an identifier. Nothing held that together end to end, and the
//! refusal outlived the gap it described.
//!
//! The fixture is a KiCad 8 board with a USB-C receptacle and a decoupling
//! capacitor, so the same file carries named pads beside numbered ones.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn run(args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    (output.status.code(), said)
}

/// The imported design, written where the repository is not.
fn imported() -> (PathBuf, String) {
    // One directory per case. All three cases call this, and it begins by
    // deleting the directory - so on a shared path they delete each other's
    // work under load. Seen once in a full workspace run from a fresh clone:
    // two of the three failed with an empty design and passed on every rerun.
    // libtest names a test thread after the case it runs.
    let case = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .replace("::", "-");
    let dir = std::env::temp_dir().join(format!("cypcb-usb-names-{case}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let out = dir.join("usb.cypcb");

    let (code, said) = run(&[
        "from-kicad",
        "tests/fixtures/usb_c_named_pads.kicad_pcb",
        "-o",
        out.to_str().expect("a path that is text"),
    ]);
    assert_eq!(
        code,
        Some(0),
        "a USB board is an ordinary board and has to import:\n{said}"
    );

    let source = std::fs::read_to_string(&out).expect("the design was written");
    (out, source)
}

#[test]
fn the_pads_come_back_with_the_names_kicad_gave_them() {
    let (_, source) = imported();

    for pad in ["A1", "A4", "A7", "B4", "S1"] {
        assert!(
            source.contains(&format!("pad {pad} ")) || source.contains(&format!("pad \"{pad}\" ")),
            "the receptacle's pad {pad} is not in the design that came back - \
             renaming a pad moves pins onto the wrong nets:\n{source}"
        );
    }

    // The capacitor beside it is numbered, and numbered pads stay numbers
    // rather than being quoted into names.
    assert!(
        source.contains("pad 1 ") && source.contains("pad 2 "),
        "a numbered pad is written as a number:\n{source}"
    );
}

#[test]
fn a_net_name_that_is_not_an_identifier_comes_back_quoted() {
    let (_, source) = imported();

    // `VBUS+` and `D-` cannot be written bare: the reader would stop at the
    // sign and the file would not be the board KiCad had.
    assert!(
        source.contains("net \"VBUS+\" {"),
        "`VBUS+` has to come back quoted:\n{source}"
    );
    assert!(
        source.contains("net \"D-\" {"),
        "`D-` has to come back quoted:\n{source}"
    );
    // And a name that is an identifier is left alone.
    assert!(
        source.contains("net GND {"),
        "`GND` needs no quotes and should not have any:\n{source}"
    );
}

#[test]
fn the_design_that_comes_back_is_one_the_checker_reads() {
    let (out, _) = imported();
    let (code, said) = run(&[
        "check",
        "--no-drc",
        out.to_str().expect("a path that is text"),
    ]);
    assert_eq!(
        code,
        Some(0),
        "the imported design has to parse - a file this project wrote and \
         cannot read is a defect in the writer:\n{said}"
    );
}
