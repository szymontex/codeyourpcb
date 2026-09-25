//! A pin drawn as several pads, and two pads drawn on one spot.
//!
//! `cargo test -p cypcb-cli --test pads_that_share_a_name_or_a_place`
//!
//! Both come from real parts. A tactile switch has two legs per contact and
//! its footprint numbers them `1`, `1`, `2`, `2`; a USB-C receptacle has four
//! shield tabs all named `SH`. The same receptacle lands A1 and B12, both
//! ground, on one spot, because the plug fits either way up.
//!
//! Until 2026-09-25 the two halves of the program disagreed about the first:
//! the router looked a pin up by name, found the first pad and routed that
//! one, while `cypcb check` asked for every pad and reported the rest as
//! unrouted. KiCad sides with the check - "The ratsnest shows missing
//! connections between same-numbered pads" unless the footprint declares its
//! duplicate numbers jumpers (KiCad 10 manual, "Jumper pads", read
//! 2026-09-25) - and this language has no such declaration, so the router now
//! routes every pad.
//!
//! And the checker reported the second as a 0.00mm mask web and a 0.000mm
//! stencil web between A1 and B12, so a receptacle routed perfectly still
//! could not export. Two pads of one net that touch are one piece of copper
//! with one opening in the mask and one in the stencil; KiCad reports a mask
//! bridge only between items "with different nets".

use std::path::Path;
use std::process::Command;

const DESIGN: &str = r#"version 1

board shared {
    size 30mm x 20mm
    layers 2
}

footprint TWO_LEGS {
    description "One contact, two legs, both named 1, one above the other"
    courtyard 2mm x 6mm
    pad 1 rect at 0mm, 2mm size 1mm x 1mm
    pad 1 rect at 0mm, -2mm size 1mm x 1mm
}

footprint STACKED {
    description "GA and GB one pad drawn twice; XC and YD two nets overlapping; GE and GF one net 0.05mm apart"
    courtyard 12mm x 2mm
    pad GE rect at -5mm, 0mm size 0.6mm x 1.2mm
    pad GF rect at -4.35mm, 0mm size 0.6mm x 1.2mm
    pad GA rect at -2mm, 0mm size 0.6mm x 1.2mm
    pad GB rect at -2mm, 0mm size 0.6mm x 1.2mm
    pad XC rect at 1.5mm, 0mm size 0.6mm x 1.2mm
    pad YD rect at 2mm, 0mm size 0.6mm x 1.2mm
}

component S1 ic "TWO_LEGS" {
    value "switch"
    at 8mm, 12mm
}

component U1 ic "STACKED" {
    value "receptacle"
    at 12mm, 5mm
}

component R1 resistor "0402" {
    value "10k"
    at 22mm, 14mm
}

net N {
    S1.1
    R1.1
}

net G {
    U1.GA
    U1.GB
    U1.GE
    U1.GF
    R1.2
}

net X {
    U1.XC
}

net Y {
    U1.YD
}
"#;

/// Where U1's GA and GB sit on the board, where XC and YD meet, and the
/// middle of the 0.05mm gap between GE and GF. Two letters, because a pin
/// named `A`, `B`, `C`, `E`, `K`, `N` or `P` alone is read as a diode,
/// transistor or polarity alias for a number.
const ONE_NET_X: f64 = 10.0;
const TWO_NETS_X: f64 = 13.75;
const ONE_NET_APART_X: f64 = 7.325;

fn cypcb(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .output()
        .expect("the binary runs")
}

/// Every row `check -o json` reports on `file`.
fn rows(file: &Path) -> Vec<serde_json::Value> {
    let output = cypcb(&["check", "-o", "json", file.to_str().unwrap()]);
    let said = String::from_utf8_lossy(&output.stdout).to_string();
    let report: serde_json::Value = serde_json::from_str(said.trim())
        .unwrap_or_else(|error| panic!("stdout should be JSON: {error}\n{said}"));
    report["violations"].as_array().cloned().unwrap_or_default()
}

fn at(rows: &[serde_json::Value], kind: &str, x: f64) -> usize {
    rows.iter()
        .filter(|row| row["kind"] == kind)
        .filter(|row| (row["x_mm"].as_f64().unwrap() - x).abs() < 0.01)
        .count()
}

fn design(who: &str) -> cypcb_fixtures::ScratchPath {
    let dir = cypcb_fixtures::scratch_dir(&format!("cypcb-shared-{who}"));
    let file = dir.join("shared.cypcb");
    std::fs::write(&file, DESIGN).expect("the scratch dir is writable");
    dir.holding(file)
}

/// S1's upper leg is level with R1's pad 1, so a route to it runs straight
/// across and never passes the lower leg: the lower one is reached only if
/// the router was asked to reach it.
#[test]
fn the_router_reaches_every_pad_the_check_asks_for() {
    let file = design("route");
    let routed = file.with_file_name("routed.cypcb");
    let output = cypcb(&[
        "route",
        "--fast",
        file.to_str().unwrap(),
        "-o",
        routed.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "route failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let rows = rows(&routed);
    let unrouted: Vec<_> = rows
        .iter()
        .filter(|row| row["kind"] == "unrouted-pin" || row["kind"] == "net-split")
        .map(|row| row["message"].as_str().unwrap_or(""))
        .collect();
    assert!(
        unrouted.is_empty(),
        "the router left pads the check wants joined: {unrouted:?}"
    );
}

#[test]
fn two_pads_of_one_net_on_one_spot_are_one_land() {
    let file = design("land");
    let rows = rows(&file);

    assert_eq!(
        at(&rows, "solder-mask-bridge", ONE_NET_X),
        0,
        "GA and GB are one ground land drawn twice, and there is no web between them: {rows:#?}"
    );
    assert_eq!(
        at(&rows, "paste-clearance", ONE_NET_X),
        0,
        "GA and GB have one stencil hole between them, not a torn web: {rows:#?}"
    );

    // The same overlap between two nets is still a fault, or the test above
    // would pass on a rule that had stopped looking.
    assert_eq!(
        at(&rows, "solder-mask-bridge", TWO_NETS_X),
        1,
        "XC and YD are two nets with no mask between them: {rows:#?}"
    );
    assert_eq!(
        at(&rows, "paste-clearance", TWO_NETS_X),
        1,
        "XC and YD share a stencil hole across two nets: {rows:#?}"
    );

    // GE and GF are one net and do not touch. Solder across their mask web
    // joins what the net joins already, so there is no mask fault; the steel
    // web between their stencil holes is 0.05mm whatever the nets, and tears.
    assert_eq!(
        at(&rows, "solder-mask-bridge", ONE_NET_APART_X),
        0,
        "GE and GF are one net: {rows:#?}"
    );
    assert_eq!(
        at(&rows, "paste-clearance", ONE_NET_APART_X),
        1,
        "GE and GF leave 0.05mm of stencil steel, one net or not: {rows:#?}"
    );
}
