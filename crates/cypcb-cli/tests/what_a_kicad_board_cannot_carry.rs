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

/// A board that states every field this project has a word for.
///
/// Written here rather than shipped as an example: it exists to be taken
/// apart, and an example is something a person copies.
const EVERYTHING: &str = r#"version 1

board everything {
    size 40mm x 30mm
    layers 4
    fab jlcpcb

    stackup {
        silk "F.SilkS" 0.01mm color "White"
        mask "F.Mask" 0.02mm material "LPI" color "Green"
        copper "F.Cu" 1oz
        prepreg "dielectric 1" 0.1mm material "7628" dk 4.5 df 0.02
        copper "In1.Cu" 0.5oz
        core "dielectric 2" 1.2mm material "FR4" dk 4.6 df 0.018
        copper "In2.Cu" 0.5oz
        prepreg "dielectric 3" 0.1mm material "7628" dk 4.5 df 0.02
        copper "B.Cu" 1oz
        mask "B.Mask" 0.02mm material "LPI" color "Green"
        silk "B.SilkS" 0.01mm color "White"

        finish "ENIG"
        edges plated
        pads castellated
        connector bevelled
        impedance controlled

        drill Top to Bottom
        drill Top to Inner1
    }
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 30mm, 10mm
}

net SIG [width 0.2mm clearance 0.25mm current 500mA impedance 50ohm] {
    R1.1
    R2.1
}

trace SIG {
    from R1.1
    to R2.1
    layer Top
    width 0.2mm
}
"#;

#[test]
fn the_trip_costs_exactly_these_three_things() {
    // The census. A board stating every field this project has a word for,
    // written out as KiCad and read back, so what the format cannot hold is a
    // list somebody maintains rather than something a reader discovers one
    // field at a time.
    let dir = scratch("everything");
    let board = dir.join("everything.cypcb");
    std::fs::write(&board, EVERYTHING).expect("the fixture is writable");

    let kicad = dir.join("everything.kicad_pcb");
    let said = to_kicad(&board, &kicad);

    // Each loss is announced, and each announcement names what was lost.
    assert!(
        said.contains("drill spans") && said.contains("Top to Inner1"),
        "{said}"
    );
    assert!(
        said.contains("fabricator this design names (jlcpcb)"),
        "{said}"
    );
    assert!(
        said.contains("SIG") && said.contains("stop checking"),
        "{said}"
    );

    let back = dir.join("back.cypcb");
    let output = cypcb()
        .arg("from-kicad")
        .arg(&kicad)
        .arg("-o")
        .arg(&back)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "reading the board back failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = std::fs::read_to_string(&back).expect("the design came back");

    // What survives, which is nearly all of it - the whole stack with its
    // names, materials, dielectric constants, loss tangents and colours, and
    // the five things a fabricator does to the board.
    for kept in [
        "layers 4",
        "finish \"ENIG\"",
        "edges plated",
        "pads castellated",
        "connector bevelled",
        "impedance controlled",
        "silk \"F.SilkS\"",
        "mask \"F.Mask\"",
        "copper \"In1.Cu\"",
        "core \"dielectric 2\"",
        "material \"7628\"",
        "dk 4.6",
        "df 0.018",
        "color \"Green\"",
        "component R1",
        "value \"10k\"",
        "trace SIG",
    ] {
        assert!(source.contains(kept), "the trip lost `{kept}`:\n{source}");
    }

    // And what it costs. Three statements, each with a rule behind it.
    assert!(
        !source.contains("drill Top"),
        "the drill spans came back, so the warning about them is stale:\n{source}"
    );
    assert!(
        !source.contains("fab "),
        "the fabricator came back, so the warning about it is stale:\n{source}"
    );
    assert!(
        !source.contains('['),
        "a net constraint came back, so the warning about them is stale:\n{source}"
    );
}
