//! A net that ends the negotiation on another net's copper is taken back.
//!
//! `cargo test -p cypcb-cli --test a_net_on_anothers_copper_is_taken_back`
//!
//! On `esp32_starter` the negotiation between the nets ends without settling:
//! two nets hold one cell, and a connection runs through the zone of a pin and
//! over the pad next to it. Before 2026-10-31 `route --fast` laid all of that
//! as copper and said `Complete`: 22 contacts of copper on copper, among them
//! CC1 on USB_DM at the USB-C connector, where the two pins sit 0.5 mm apart in
//! one 0.627 mm cell.
//!
//! The router now strips each net that holds a cell with a net routed before
//! it, or whose path crosses another net's pad, and routes it again where it
//! may touch nothing. A connection with no such path stays open, and the run
//! says so and says which.

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

/// The log with its colour codes taken out, which the logger writes whether or
/// not a terminal is reading.
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

struct Routed {
    said: String,
    contacts: Vec<String>,
}

fn route_fast() -> Routed {
    let dir = cypcb_fixtures::scratch_dir("cypcb-esp32-strip");
    let routed = dir.join("esp32_starter.routed.cypcb");
    let output = cypcb()
        .env("RUST_LOG", "cypcb_autoroute=info")
        .args(["route", "--fast", "-o"])
        .arg(&routed)
        .arg(fixture())
        .output()
        .expect("the binary runs");
    assert!(output.status.success(), "routing failed");
    let said = plain(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));

    let check = cypcb()
        .args(["check", "-o", "json"])
        .arg(&routed)
        .output()
        .expect("the binary runs");
    let report: serde_json::Value =
        serde_json::from_slice(&check.stdout).expect("check prints JSON");
    let contacts = report["violations"]
        .as_array()
        .expect("a list of violations")
        .iter()
        .filter(|v| v["kind"] == "clearance" && v["actual_mm"] == 0.0)
        .map(|v| v["message"].as_str().unwrap_or_default().to_string())
        .collect();
    Routed { said, contacts }
}

#[test]
fn cc1_no_longer_lies_on_usb_dm() {
    let routed = route_fast();
    let on_each_other: Vec<&String> = routed
        .contacts
        .iter()
        .filter(|m| m.contains("'CC1'") && m.contains("'USB_DM'"))
        .collect();
    assert!(
        on_each_other.is_empty(),
        "CC1 and USB_DM touch again: {on_each_other:?}"
    );
}

#[test]
fn the_contacts_fall_and_none_is_between_two_traces() {
    let routed = route_fast();
    // 22 on 2026-10-30, 9 of them between two traces. 10 since 2026-10-31,
    // none between two traces; what is left is a trace or a via against a pad.
    assert_eq!(
        routed.contacts.len(),
        10,
        "the contacts of copper on copper: {:#?}",
        routed.contacts
    );
    let between_traces = routed
        .contacts
        .iter()
        .filter(|m| m.starts_with("trace") && m.contains("↔ trace"))
        .count();
    assert_eq!(between_traces, 0, "{:#?}", routed.contacts);
}

#[test]
fn every_net_taken_back_is_said_with_where_and_what_came_of_it() {
    let routed = route_fast();
    let said = &routed.said;
    // USB_DM holds a cell with USB_DP and has no path that touches nothing.
    assert!(
        said.contains("Net stripped") && said.contains("net=USB_DM"),
        "the run does not say USB_DM was taken back:\n{said}"
    );
    assert!(
        said.contains("holds the cell with USB_DP"),
        "the run does not say why:\n{said}"
    );
    assert!(
        said.contains("Stripped net routed again") && said.contains("unrouted connections left"),
        "the run does not say what came of it:\n{said}"
    );
    assert!(
        said.contains("routed on tier 1"),
        "no net found a path that touches nothing, where four did:\n{said}"
    );
    assert!(
        said.contains("could not be routed"),
        "a connection left open has to be said at the end:\n{said}"
    );
}
