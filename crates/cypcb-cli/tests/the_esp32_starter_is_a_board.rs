//! The ESP32-S3 starter board parses, carries the datasheet's pins, and routes.
//!
//! `cargo test -p cypcb-cli --test the_esp32_starter_is_a_board`
//!
//! `tests/fixtures/benchmark/esp32_starter.cypcb` is the first benchmark board
//! written in this language by hand rather than read from KiCad, so it is the
//! one that finds out what a designer has to say and cannot. It is also the
//! board compared against other tools, which makes its pins worth guarding:
//! a board that parses but puts USB on the wrong module pins still parses.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark/esp32_starter.cypcb")
}

fn parsed() -> serde_json::Value {
    let output = cypcb()
        .arg("parse")
        .arg(fixture())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "the board does not parse:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse prints JSON")
}

/// The net a pin of a component is on, as the parser read it.
fn net_of(board: &serde_json::Value, refdes: &str, pin: &str) -> Option<String> {
    board["components"]
        .as_array()?
        .iter()
        .find(|c| c["refdes"] == refdes)?["pins"]
        .as_array()?
        .iter()
        .find(|p| p["pin"] == pin)
        .and_then(|p| p["net"].as_str())
        .map(str::to_string)
}

#[test]
fn every_part_on_the_board_has_a_footprint() {
    let board = parsed();
    let components = board["components"].as_array().expect("a component list");
    assert_eq!(components.len(), 18, "the starter board has 18 parts");
    let unknown: Vec<_> = components
        .iter()
        .filter(|c| c["footprint_known"] != true)
        .map(|c| c["refdes"].to_string())
        .collect();
    assert!(unknown.is_empty(), "parts with no footprint: {unknown:?}");
}

#[test]
fn the_module_is_wired_to_the_pins_its_datasheet_names() {
    // ESP32-S3-WROOM-1 datasheet v1.8, Table 3-1: 2 is 3V3, 3 is EN, 13 is
    // IO19 (USB_D-), 14 is IO20 (USB_D+), 27 is IO0, 1, 40 and 41 are GND.
    let board = parsed();
    for (pin, net) in [
        ("1", "GND"),
        ("2", "V3V3"),
        ("3", "EN"),
        ("13", "USB_DM"),
        ("14", "USB_DP"),
        ("27", "BOOT"),
        ("40", "GND"),
        ("41", "GND"),
    ] {
        assert_eq!(
            net_of(&board, "U1", pin).as_deref(),
            Some(net),
            "module pin {pin} should be on {net}"
        );
    }
    // GCT USB4105 drawing: A6/B6 are D+, A7/B7 are D-, A5 and B5 are CC.
    for (pin, net) in [
        ("A6", "USB_DP"),
        ("B6", "USB_DP"),
        ("A7", "USB_DM"),
        ("B7", "USB_DM"),
        ("A5", "CC1"),
        ("B5", "CC2"),
    ] {
        assert_eq!(
            net_of(&board, "J1", pin).as_deref(),
            Some(net),
            "receptacle pin {pin} should be on {net}"
        );
    }
    // AP2112K in SOT25, DS39724: 1 VIN, 2 GND, 3 EN, 5 VOUT.
    for (pin, net) in [("1", "VBUS"), ("2", "GND"), ("3", "VBUS"), ("5", "V3V3")] {
        assert_eq!(
            net_of(&board, "U2", pin).as_deref(),
            Some(net),
            "regulator pin {pin} should be on {net}"
        );
    }
}

#[test]
fn the_board_routes() {
    let dir = cypcb_fixtures::scratch_dir("cypcb-esp32-starter");
    let routed = dir.join("esp32_starter.routed.cypcb");
    let output = cypcb()
        .arg("route")
        .arg(fixture())
        .arg("--fast")
        .arg("-o")
        .arg(&routed)
        .output()
        .expect("the binary runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "routing failed:\n{said}");

    let text = std::fs::read_to_string(&routed).expect("the routed board was written");
    for net in ["USB_DP", "USB_DM", "V3V3", "EN"] {
        assert!(
            text.contains(&format!("trace {net} ")),
            "the routed board carries no trace on {net}"
        );
    }

    let check = cypcb()
        .arg("check")
        .arg(&routed)
        .output()
        .expect("the binary runs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(
        report.contains("DRC violation(s)"),
        "the routed board did not reach the rule check:\n{report}"
    );
}
