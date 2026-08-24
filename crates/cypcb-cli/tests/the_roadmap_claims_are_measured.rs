//! The roadmap claims, each measured by the command that shows it.
//!
//! `cargo test -p cypcb-cli --test the_roadmap_claims_are_measured`
//!
//! `docs/TRACKER.md`'s phase map says of P4: copper pour, KiCad in and out,
//! modules, imports and assertions "are done and have tests". `interface` sat
//! in the same sentence saying it built nothing, and it was enforced - a claim
//! that had travelled from that line into a completion exclusion and into a
//! test's doc comment without anybody running a command.
//!
//! So the sentence gets a test. Each case here is one thing the phase map
//! asserts, checked against the binary rather than against the file.

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

fn example(name: &str) -> PathBuf {
    repo_root().join("examples").join(name)
}

/// Run `check` and return everything it said.
fn check(board: &Path) -> String {
    let output = cypcb()
        .arg("check")
        .arg(board)
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn a_pour_is_filled_and_its_islands_are_reported() {
    // `examples/pour-island.cypcb` is a plane cut in two by a trace, and the
    // half no thermal spoke reaches is copper connected to nothing.
    let said = check(&example("pour-island.cypcb"));
    assert!(
        said.contains("pour-island: 1"),
        "the rule that finds orphaned copper has to fire on the example written for it:\n{said}"
    );
    assert!(
        said.contains("copper 30.000mm x 14.773mm"),
        "and it has to say how much copper is stranded:\n{said}"
    );
}

#[test]
fn a_module_places_the_parts_it_holds() {
    // Two LED modules and a power supply, each instantiated with its own
    // prefix, which is what makes a module worth having.
    let output = cypcb()
        .arg("parse")
        .arg(example("v2-modules.cypcb"))
        .output()
        .expect("the binary runs");
    assert!(output.status.success(), "parsing the example has to work");
    let json = String::from_utf8_lossy(&output.stdout).to_string();

    for refdes in ["LED_PWR_D1", "LED_PWR_R1", "PSU_C_IN"] {
        assert!(
            json.contains(refdes),
            "a module instance names its parts after itself, and `{refdes}` is missing:\n{json}"
        );
    }
}

#[test]
fn an_import_brings_a_module_with_it() {
    // The same, across a file boundary: `v2-imports.cypcb` places `Divider`
    // twice and `LedDriver` once, and none of the three is defined in it.
    let output = cypcb()
        .arg("parse")
        .arg(example("v2-imports.cypcb"))
        .output()
        .expect("the binary runs");
    assert!(output.status.success(), "parsing the example has to work");
    let json = String::from_utf8_lossy(&output.stdout).to_string();

    for refdes in ["DIV_A_RTOP", "DIV_B_RBOT", "STATUS_D1"] {
        assert!(
            json.contains(refdes),
            "`{refdes}` comes from an imported module and is not in the file:\n{json}"
        );
    }
}

#[test]
fn an_assertion_that_is_false_is_reported() {
    // Both halves, because either alone proves nothing: the example asserts
    // things that hold and reports no assertion, and the same file with one
    // figure moved reports exactly one.
    let honest = check(&example("v2-constraints.cypcb"));
    assert!(
        !honest.contains("assertion:"),
        "everything this example asserts is true of it:\n{honest}"
    );

    let source = std::fs::read_to_string(example("v2-constraints.cypcb")).expect("the example");
    let broken = source.replace(
        "assert C1.value within 100nF to 220nF",
        "assert C1.value within 1nF to 2nF",
    );
    assert_ne!(
        broken, source,
        "the assertion this test moves is still there"
    );

    let dir = std::env::temp_dir().join("cypcb-roadmap-assert");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("moved.cypcb");
    std::fs::write(&board, broken).expect("the copy is writable");

    let said = check(&board);
    assert!(
        said.contains("assertion: 1"),
        "a 100nF part is not within 1nF to 2nF and the checker has to say so:\n{said}"
    );
}

#[test]
fn a_module_is_held_to_the_interface_it_signs() {
    // The claim that was wrong in the phase map for weeks, kept here so it
    // cannot go wrong quietly again.
    let dir = std::env::temp_dir().join("cypcb-roadmap-interface");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("iface.cypcb");
    std::fs::write(
        &board,
        r#"version 1

board t {
    size 20mm x 20mm
    layers 2
}

interface I2C {
    pin SDA
    pin SCL
}

module Sensor {
    implements I2C

    pin SDA

    component U1 ic "0402" {
        value "s"
        at 5mm, 5mm
    }
}
"#,
    )
    .expect("the fixture is writable");

    let said = check(&board);
    assert!(
        said.contains("interface_not_satisfied") || said.contains("without pin SCL"),
        "a module that signs an interface and skips one of its pins has to be told:\n{said}"
    );
}
