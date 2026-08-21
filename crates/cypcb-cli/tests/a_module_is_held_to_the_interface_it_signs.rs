//! A module is held to the interface it signs.
//!
//! `cargo test -p cypcb-cli --test a_module_is_held_to_the_interface_it_signs`
//!
//! `implements I2C` is a promise about which pins a module has, and
//! `examples/v2-interfaces.cypcb` described the checker keeping modules to it
//! while having no board, no instance, and nothing for the checker to do -
//! `cypcb check` answered "declares no board and places no components:
//! nothing was checked".
//!
//! The behaviour it described turned out to be real and unpinned. These run
//! the binary, because half of what is claimed is the **exit code**: a design
//! that does not add up has to fail the command, not print a note and succeed.
//!
//! It is a sync error rather than a DRC violation, and that distinction is
//! worth keeping. A board with a fault on it is still a board; a module that
//! does not keep its word is a design nothing downstream should be asked to
//! make sense of.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Run `cypcb check` on a written-out design and return its status and report.
fn check(name: &str, source: &str) -> (i32, String) {
    let dir = std::env::temp_dir().join("cypcb-interface-contract");
    std::fs::create_dir_all(&dir).expect("a place to put the design");
    let path = dir.join(format!("{name}.cypcb"));
    std::fs::write(&path, source).expect("the design is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&path)
        .output()
        .expect("the CLI runs");
    (
        output.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

/// An I2C interface and a module signing it, with `pins` as the module's own.
fn design(pins: &str) -> String {
    format!(
        "version 1\n\n\
         interface I2C {{\n    pin SDA\n    pin SCL\n}}\n\n\
         module Sensor {{\n    implements I2C\n\n    \
         component U1 ic \"SOIC-8\" {{\n        value \"TMP102\"\n        at 0mm, 0mm\n    }}\n\n\
         {pins}}}\n"
    )
}

#[test]
fn a_missing_pin_fails_the_command_and_names_it() {
    let (status, report) = check("missing", &design("    pin SCL\n"));
    assert_eq!(
        status, 1,
        "a design that does not add up has to fail:\n{report}"
    );
    assert!(
        report.contains("interface_not_satisfied"),
        "and say which kind of error it is:\n{report}"
    );
    // All three of these are what a reader needs to fix it: which module, which
    // contract, which pin.
    assert!(report.contains("Sensor"), "{report}");
    assert!(report.contains("I2C"), "{report}");
    assert!(report.contains("SDA"), "{report}");
}

#[test]
fn a_module_that_keeps_its_word_is_not_reported() {
    let (_, report) = check("kept", &design("    pin SDA\n    pin SCL\n"));
    assert!(
        !report.contains("interface_not_satisfied"),
        "a contract that is kept is not a fault:\n{report}"
    );
}

#[test]
fn each_missing_pin_is_named_rather_than_the_first_one_only() {
    // A module short two pins should not need two runs to find that out.
    let (status, report) = check("both", &design("    pin VCC\n"));
    assert_eq!(status, 1);
    assert!(report.contains("SDA"), "{report}");
    assert!(report.contains("SCL"), "{report}");
}

#[test]
fn the_example_keeps_the_contracts_it_signs() {
    // `examples/v2-interfaces.cypcb` gained a board and two instances so that
    // the contracts above are exercised rather than only written down. This is
    // what stops it drifting back into a file with nothing to check.
    let path = repo_root().join("examples/v2-interfaces.cypcb");
    let source = std::fs::read_to_string(&path).expect("the example is readable");
    assert!(
        source.contains("board sensor_node"),
        "the example needs a board, or the checker has nothing to hold it to"
    );
    assert!(
        source.contains("use TemperatureSensor") && source.contains("use OledDisplay"),
        "and an instance of each module"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&path)
        .output()
        .expect("the CLI runs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !report.contains("interface_not_satisfied"),
        "the example's own modules have to keep their contracts:\n{report}"
    );
    assert!(
        !report.contains("nothing was checked"),
        "and the file has to give the checker something to do:\n{report}"
    );
    // Unrouted pins are expected on a board nobody routed. Copper that
    // overlaps copper is not, and the module parts sat close enough to do it.
    assert!(
        !report.contains("clearance at"),
        "the example's own parts must not collide:\n{report}"
    );
}
