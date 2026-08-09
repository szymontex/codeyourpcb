//! Two nets carrying one signal have to be the same length.
//!
//! `cargo test -p cypcb-cli --test a_pair_that_does_not_match_is_reported`
//!
//! `diffpair USB { USB_DP USB_DM }` parsed into the AST and was read by
//! nothing, and `length_match_tolerance` sat in every fab preset and was read
//! by nothing either. A design could declare a pair, route one half twice as
//! long as the other, and be told the board was fine.

use std::process::Command;

/// Two nets between the same pair of parts, one routed the long way round.
fn board(detour_mm: f64) -> String {
    format!(
        r#"version 1

board pair {{
    size 60mm x 40mm
    layers 2
}}

component J1 connector "PIN-HDR-1x2" {{
    at 10mm, 20mm
}}

component U1 ic "SOIC-8" {{
    value "USB"
    at 40mm, 20mm
}}

net USB_DP {{
    J1.1
    U1.1
}}

net USB_DM {{
    J1.2
    U1.2
}}

diffpair USB {{
    USB_DP
    USB_DM
}}

trace USB_DP {{
    layer Top
    width 0.25mm
    path 10mm,19mm -> 40mm,19mm
}}

trace USB_DM {{
    layer Top
    width 0.25mm
    path 10mm,21mm -> {end}mm,21mm -> 40mm,21mm
}}
"#,
        end = 40.0 + detour_mm / 2.0
    )
}

fn check(source: &str, name: &str) -> String {
    let dir = std::env::temp_dir().join("cypcb-diffpair");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let file = dir.join(format!("{name}.cypcb"));
    std::fs::write(&file, source).expect("the board is written");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check"])
        .arg(&file)
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn a_pair_routed_the_long_way_round_is_reported() {
    // 10mm of detour, against JLCPCB's 0.5mm length-match tolerance.
    let report = check(&board(10.0), "skewed");

    assert!(
        report.contains("diff-pair-skew"),
        "the halves are 10mm apart and nothing said so:\n{report}"
    );
    assert!(
        report.contains("USB_DP") && report.contains("USB_DM"),
        "the message has to name both halves:\n{report}"
    );
}

#[test]
fn a_pair_that_matches_is_silent() {
    // Both halves run straight across: same length, nothing to report.
    let report = check(&board(0.0), "matched");

    assert!(
        !report.contains("diff-pair-skew"),
        "the halves are the same length and something complained:\n{report}"
    );
}

#[test]
fn a_pair_naming_a_net_that_is_not_there_is_reported() {
    // The typo that would otherwise turn the check off silently.
    let source = board(10.0).replace("    USB_DM\n}", "    USB_DN\n}");
    let report = check(&source, "typo");

    assert!(
        report.contains("USB_DN") && report.contains("not a net"),
        "a pair naming a net the board does not have has to say so:\n{report}"
    );
}
