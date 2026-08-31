//! A layer the stack does not have is reported by the stack, not by silence.
//!
//! `cargo test -p cypcb-cli --test a_layer_the_stack_does_not_have_is_reported_by_the_stack`
//!
//! `ImpedanceRule` skips a trace whose layer it cannot place in the stackup -
//! `copper_index` returns nothing - and says nothing about it. That is the
//! same shape as the exception fixed one commit earlier, where the rule
//! returned nothing for a board with no stack at all and named another rule as
//! the one that would speak. That other rule did not.
//!
//! This one does. The only way a trace reaches a layer the stack does not have
//! is a stack that disagrees with the board, and `StackupRule` reports that by
//! name. So the silence is covered - and covered is a thing to check, not to
//! assert in a comment.

use std::process::Command;

/// A four-layer board whose stackup describes two coppers, with a net asking
/// for an impedance on the inner layer that stack does not have.
///
/// The dielectric is 0.1mm on purpose: a 0.2mm trace over it is nowhere near
/// the 90 ohm the net asks for, so a rule that quietly measured some other
/// layer instead of skipping this one would report a miss and be caught.
const MISMATCHED: &str = r#"version 1

board mismatch {
    size 30mm x 20mm
    layers 4
    fab jlcpcb

    stackup {
        copper "F.Cu" 1oz
        core "dielectric 1" 0.10mm material "FR4" dk 4.6 df 0.018
        copper "B.Cu" 1oz

        finish "HASL"
    }
}

component J1 connector "PIN-HDR-1x2" {
    at 5mm, 8mm
}

component J2 connector "PIN-HDR-1x2" {
    at 25mm, 8mm
}

net SIG [impedance 90ohm] {
    J1.1
    J2.1
}

trace SIG {
    from J1.1
    to J2.1
    layer Inner2
    width 0.2mm
}
"#;

fn check(design: &str, name: &str) -> String {
    let file = std::env::temp_dir().join(name);
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
    std::fs::remove_file(&file).ok();
    assert!(!report.trim().is_empty(), "the checker printed nothing");
    report
}

#[test]
fn the_stack_that_cannot_hold_the_trace_is_the_thing_reported() {
    let report = check(MISMATCHED, "cypcb-mismatched-stack.cypcb");

    // The reason the impedance is not measured, said by the rule that owns it.
    assert!(
        report.contains("board says 4 copper layers and the stackup describes 2"),
        "the stack that disagrees with the board is not reported:\n{report}"
    );
    // And the impedance rule stays quiet, because repeating it per trace would
    // name the same fault twice.
    assert!(
        !report.contains("impedance"),
        "the impedance rule speaks about a layer the stack does not have:\n{report}"
    );
}

#[test]
fn the_same_board_with_a_stack_that_matches_is_measured() {
    // The control: give the stack the four coppers the board claims and the
    // trace lands on a layer that exists, so the rule measures it instead of
    // skipping it. A miss here is a number, not silence.
    let matched = MISMATCHED.replace(
        r#"        copper "F.Cu" 1oz
        core "dielectric 1" 0.10mm material "FR4" dk 4.6 df 0.018
        copper "B.Cu" 1oz"#,
        r#"        copper "F.Cu" 1oz
        prepreg "dielectric 1" 0.2mm material "7628" dk 4.5 df 0.02
        copper "In1.Cu" 0.5oz
        core "dielectric 2" 1.065mm material "FR4" dk 4.6 df 0.018
        copper "In2.Cu" 0.5oz
        prepreg "dielectric 3" 0.2mm material "7628" dk 4.5 df 0.02
        copper "B.Cu" 1oz"#,
    );
    assert!(
        matched.contains("In2.Cu"),
        "the four-copper stack was not substituted, so this case proves nothing"
    );

    let report = check(&matched, "cypcb-matched-stack.cypcb");
    assert!(
        !report.contains("board says 4 copper layers"),
        "the stack still disagrees with the board:\n{report}"
    );
    assert!(
        report.contains("impedance"),
        "a 0.2mm trace on an inner layer of this stack is nowhere near 90 ohm, \
         and the rule said nothing about it:\n{report}"
    );
}
