//! What a `.kicad_pcb` cannot carry, said out loud.
//!
//! `cargo test -p cypcb-cli --test what_a_kicad_board_cannot_carry`
//!
//! A design states which spans its fabricator drills - `drill Top to Bottom`,
//! `drill Top to Inner1` - and `ViaSpanRule` holds the board's vias to that
//! list. KiCad keeps no such list in the board file: a via there carries its
//! own two layers, and which spans a build makes lives in the project's design
//! rules rather than in the `(setup ...)` this writer fills.
//!
//! So the statement is dropped, and the same board checked before and after a
//! round trip gets two answers. That is a property of the format rather than a
//! bug to fix here - what was a bug is that it happened without a word.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
        .join(name)
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-kicad-loss-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    dir
}

/// Write a design out as a KiCad board, returning what it said on stderr.
fn to_kicad(board: &Path, out: &Path) -> String {
    let output = cypcb()
        .arg("to-kicad")
        .arg(board)
        .arg("-o")
        .arg(out)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "writing the KiCad board failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn a_design_that_states_its_drill_spans_is_told_they_are_dropped() {
    // `examples/rigid-flex.cypcb` states `drill Top to Bottom`: one cycle, one
    // span, which is the whole point of a flex build.
    let dir = scratch("stated");
    let said = to_kicad(
        &example("rigid-flex.cypcb"),
        &dir.join("rigid-flex.kicad_pcb"),
    );

    assert!(
        said.contains("drill spans"),
        "the design states a span the format cannot hold and nothing said so:\n{said}"
    );
    assert!(
        said.contains("Top to Bottom"),
        "and it has to name which one:\n{said}"
    );
}

#[test]
fn a_design_that_states_none_is_left_alone() {
    // The half that keeps the other from being noise. `examples/blink.cypcb`
    // states no stackup at all, so there is nothing to drop and nothing to say.
    let dir = scratch("silent");
    let said = to_kicad(&example("blink.cypcb"), &dir.join("blink.kicad_pcb"));

    assert!(
        !said.contains("drill spans"),
        "a board that states no spans has none to lose:\n{said}"
    );
}

#[test]
fn a_stack_that_states_no_spans_is_left_alone_too() {
    // The case the example above cannot make: a board with a stackup and no
    // `drill` line in it. Without this, a warning that fired on every stack
    // rather than on every stated span would still pass - no shipped example
    // has a stack without one.
    let dir = scratch("stack-no-spans");
    let board = dir.join("plain-stack.cypcb");
    std::fs::write(
        &board,
        [
            "version 1",
            "",
            "board plain {",
            "    size 20mm x 20mm",
            "    layers 2",
            "",
            "    stackup {",
            "        copper 1oz",
            "        core 1.5mm material \"FR4\" dk 4.5",
            "        copper 1oz",
            "        finish \"HASL\"",
            "    }",
            "}",
            "",
        ]
        .join("\n"),
    )
    .expect("the fixture is writable");

    let said = to_kicad(&board, &dir.join("plain-stack.kicad_pcb"));
    assert!(
        !said.contains("drill spans"),
        "this stack states no spans, so it loses none:\n{said}"
    );
}

#[test]
fn the_round_trip_loses_the_spans_and_keeps_the_stack() {
    // What the warning is about, measured rather than asserted from the code:
    // the stack itself survives - coverlay, foil, core, stiffener, finish -
    // and the drill spans do not.
    let dir = scratch("round-trip");
    let kicad = dir.join("rigid-flex.kicad_pcb");
    to_kicad(&example("rigid-flex.cypcb"), &kicad);

    let back = dir.join("rigid-flex-back.cypcb");
    let output = cypcb()
        .arg("from-kicad")
        .arg(&kicad)
        .arg("-o")
        .arg(&back)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "reading the KiCad board back failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let source = std::fs::read_to_string(&back).expect("the design came back");
    assert!(
        source.contains("stackup {") && source.contains("coverlay") && source.contains("stiffener"),
        "the stack itself does survive the trip:\n{source}"
    );
    assert!(
        !source.contains("drill "),
        "the spans do not, which is what the warning is about:\n{source}"
    );
}
