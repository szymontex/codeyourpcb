//! The three IPC classes are three different tables, and the binary can tell
//! them apart.
//!
//! `cargo test -p cypcb-cli --test the_ipc_classes_are_not_one_table`
//!
//! `ipc_class1` and `ipc_class3` were the last two presets no test named -
//! found by counting every preset the binary offers against the suite. The
//! risk with a family of tables written from one document is that two of them
//! end up the same, or the wrong way round, and nothing notices: they are read
//! by name and nobody checks that the name changed anything.
//!
//! What separates them here is the trace width each is made at. A class 1
//! board is a cheap process and its floor is **0.2mm**; class 2 is 0.15mm and
//! class 3 is 0.1mm, because a board built to the strictest performance class
//! is also built on the finest process. The demands that go the other way -
//! more copper around a hole, more margin at an edge - are what the class is
//! for, and they are checked by their own rules.
//!
//! These are this tool's reading of a document that is not public, which the
//! report says out loud on every one of the three; that note has its own test
//! in `whose_table_is_checking_this_board`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A board whose one trace is 0.15mm wide: the width class 2 is made at.
const BOARD: &str = r#"version 1

board grades {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 5mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 25mm, 10mm
}

net SIG {
    R1.1
    R2.1
}

trace SIG {
    from R1.1
    to R2.1
    layer Top
    width 0.15mm
}
"#;

/// One case's own directory: cargo runs these four at the same time, and the
/// first draft had them share one per preset - so the case that grades all
/// three wiped what another was reading, and it failed inside the gate while
/// passing alone.
fn check(who: &str, preset: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-ipc-{who}-{preset}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, BOARD).expect("the fixture is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", "--preset", preset])
        .arg(&board)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn class_one_is_made_on_a_coarser_process_than_the_trace_asks_for() {
    let said = check("coarse", "ipc_class1");
    assert!(
        said.contains("Trace width violation: 0.150mm actual, 0.200mm minimum"),
        "a class 1 board is etched at 0.2mm and this trace is 0.15mm:\n{said}"
    );
}

#[test]
fn class_two_is_made_at_exactly_this_width() {
    let said = check("exact", "ipc_class2");
    assert!(
        !said.contains("trace-width"),
        "0.15mm is the width class 2 states, and a floor is a floor:\n{said}"
    );
}

#[test]
fn class_three_is_finer_still() {
    let said = check("fine", "ipc_class3");
    assert!(
        !said.contains("trace-width"),
        "class 3 is etched at 0.1mm, so this trace is comfortable:\n{said}"
    );
}

#[test]
fn the_three_reports_name_the_table_they_used() {
    // A board graded against the wrong table is worse than one nobody graded,
    // so the name travels with the count.
    for (preset, expected) in [
        ("ipc_class1", "against ipc_class1"),
        ("ipc_class2", "against ipc_class2"),
        ("ipc_class3", "against ipc_class3"),
    ] {
        let said = check("named", preset);
        assert!(
            said.contains(expected),
            "`--preset {preset}` has to say so in the report:\n{said}"
        );
    }
}
