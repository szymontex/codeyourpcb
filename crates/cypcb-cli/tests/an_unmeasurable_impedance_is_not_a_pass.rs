//! A net that asks for an impedance nobody can measure is not a net that passed.
//!
//! `cargo test -p cypcb-cli --test an_unmeasurable_impedance_is_not_a_pass`
//!
//! `ImpedanceRule` compares what a net asks for against what the stack
//! delivers, and says so when a layer's surroundings cannot be described -
//! "Not checked - not passed", which is the whole point of the rule. It made
//! one exception, and it was the largest one: a board that states **no**
//! stackup returned nothing at all, on the grounds that `stackup`'s own rule
//! would speak instead.
//!
//! It does not. That rule reports nothing when a design states no stack,
//! because taking the fabricator's is ordinary and not a fault. So a USB pair
//! asking for 90 ohm on a stackless board passed in silence, and silence from
//! a checker reads as a pass.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two directories above this crate")
}

fn check(example: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(repo_root().join("examples").join(example))
        .output()
        .expect("the binary runs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!report.trim().is_empty(), "the checker printed nothing");
    report
}

/// The example with its `stackup` block taken out, so the only difference
/// between the two cases below is whether the board says what holds its
/// traces.
fn without_the_stackup(design: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for line in design.lines() {
        if depth > 0 {
            depth += line.matches('{').count();
            depth -= line.matches('}').count();
            continue;
        }
        if line.trim_start().starts_with("stackup {") {
            depth = 1;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[test]
fn the_pair_is_measured_against_the_stack_the_board_states() {
    // `usb-diff-pair.cypcb` states 0.41mm of FR4 under 1oz copper, and its
    // 0.2mm traces really are near the 90 ohm both nets ask for - so the
    // checker says nothing about impedance, having measured it.
    let report = check("usb-diff-pair.cypcb");
    assert!(
        !report.contains("impedance"),
        "the pair misses the impedance its own stack gives it:\n{report}"
    );
}

#[test]
fn the_same_pair_without_a_stack_is_named_rather_than_passed() {
    // The control, and the defect. Take the `stackup` block out of that same
    // board and nothing can be asked what it delivers - which used to be
    // reported as nothing at all.
    let design = std::fs::read_to_string(repo_root().join("examples/usb-diff-pair.cypcb"))
        .expect("the example is readable");
    let stackless = without_the_stackup(&design);
    assert!(
        !stackless.contains("dielectric 1"),
        "the stackup was not removed, so this case proves nothing"
    );

    let file = std::env::temp_dir().join("cypcb-stackless-pair.cypcb");
    std::fs::write(&file, stackless).expect("a design to check");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&file)
        .output()
        .expect("the binary runs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        report.contains("net 'USB_DP' asks for 90ohm and this design states no stackup"),
        "the pair that cannot be measured is not named:\n{report}"
    );
    assert!(
        report.contains("Not checked - not passed"),
        "the report does not say the difference between unchecked and passed:\n{report}"
    );
    assert!(
        report.contains("impedance: 2"),
        "one line per net, both halves:\n{report}"
    );
    std::fs::remove_file(&file).ok();
}

#[test]
fn a_net_routed_in_two_segments_is_named_once() {
    // One report per net, not per segment. `usb-diff-pair.cypcb` cannot show
    // the difference - each of its nets is one trace - so the design that can
    // is written here: one net across three pins, two segments, no stackup.
    let design = r#"version 1

board two_segments {
    size 30mm x 20mm
    layers 2
    fab jlcpcb
}

component J1 connector "PIN-HDR-1x2" {
    at 5mm, 8mm
}

component J2 connector "PIN-HDR-1x2" {
    at 15mm, 8mm
}

component J3 connector "PIN-HDR-1x2" {
    at 25mm, 8mm
}

net SIG [impedance 90ohm] {
    J1.1
    J2.1
    J3.1
}

trace SIG {
    from J1.1
    to J2.1
    layer Top
    width 0.2mm
}

trace SIG {
    from J2.1
    to J3.1
    layer Top
    width 0.2mm
}
"#;

    let file = std::env::temp_dir().join("cypcb-two-segments.cypcb");
    std::fs::write(&file, design).expect("a design to check");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&file)
        .output()
        .expect("the binary runs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        report
            .matches("asks for 90ohm and this design states no stackup")
            .count(),
        1,
        "the net is routed in two segments and should be named once:\n{report}"
    );
    std::fs::remove_file(&file).ok();
}
