//! A file with no board is not a board that passed.
//!
//! `cargo test -p cypcb-cli --test a_board_that_is_not_there_does_not_pass`
//!
//! `examples/v2-interfaces.cypcb` declares four interfaces and nothing else -
//! no board, no components, no nets - and `cypcb check` answered:
//!
//! ```text
//! OK: examples/v2-interfaces.cypcb passed DRC against jlcpcb_2layer in 0ms
//! ```
//!
//! Every rule skips quietly when there is no board size; `EdgeClearanceRule`
//! says so in its own doc comment. So the checker ran, checked nothing, and
//! called it a pass. That is fine for a file which is honestly a library of
//! interface declarations. It is not fine for the case it is indistinguishable
//! from: a design whose `board` block failed to parse, or whose import did not
//! resolve, which gets the same green line as a board that was checked.

use std::path::PathBuf;
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn write(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("cypcb-no-board");
    std::fs::create_dir_all(&dir).expect("a place to work");
    let path = dir.join(name);
    std::fs::write(&path, source).expect("the file is writable");
    path
}

fn run(path: &PathBuf) -> (bool, String) {
    let output = cypcb()
        .arg("check")
        .arg(path)
        .output()
        .expect("the binary runs");
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

#[test]
fn a_file_with_nothing_on_it_says_nothing_was_checked() {
    // A library of declarations is a legitimate file, so this is not an error.
    // It must simply not claim a DRC pass it never performed.
    let path = write(
        "declarations-only.cypcb",
        "version 1\n\ninterface I2C {\n    pin SDA\n    pin SCL\n}\n",
    );
    let (ok, out) = run(&path);

    assert!(ok, "a file of declarations is not an error:\n{out}");
    assert!(
        !out.contains("passed DRC"),
        "it claimed a check it never ran:\n{out}"
    );
    assert!(
        out.contains("nothing was checked"),
        "it has to say that nothing was checked:\n{out}"
    );
}

#[test]
fn components_with_no_board_are_an_error() {
    // This is the case worth catching: parts placed on a board that is not
    // there. Before, it read as a clean pass.
    let path = write(
        "parts-without-a-board.cypcb",
        "version 1\n\ncomponent R1 resistor \"0402\" {\n    value \"10k\"\n    at 5mm, 5mm\n}\n",
    );
    let (ok, out) = run(&path);

    assert!(
        !ok,
        "a component placed on no board came back a success:\n{out}"
    );
    assert!(
        out.contains("declares no board"),
        "the reason has to name what is missing:\n{out}"
    );
    assert!(
        out.contains('1'),
        "and how many parts are waiting for it:\n{out}"
    );
}

#[test]
fn a_real_board_is_still_checked() {
    // The half that must not change. `panel-mount` has four mounting holes and
    // real copper, and it has to keep reaching the design rule check.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples/panel-mount.cypcb");
    let (_, out) = run(&path.to_path_buf());

    assert!(
        out.contains("DRC violation") || out.contains("passed DRC"),
        "a board with parts on it has to reach the checker:\n{out}"
    );
    assert!(
        !out.contains("nothing was checked"),
        "a real board was treated as empty:\n{out}"
    );
}
