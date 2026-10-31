//! Repair works on a board with connections open, and trades none of them for a short.
//!
//! `cargo test --release -p cypcb-cli --test repair_on_a_board_with_open_connections`
//!
//! Since 2026-10-31 the router strips a net that ends its negotiation on another
//! net's copper and leaves a connection open where no path keeps clear. Until
//! the same day repair skipped a board with a connection open. Opened with the
//! ranking alone it routed the stripped nets across other copper again: shorts
//! on `esp32_starter` went 7 -> 12, on `mains-sequencer` 11 -> 61 and on
//! `qfp_fanout` 14 -> 34. Repair now
//! routes such a board again with the strip on and then off, and keeps an
//! attempt only if it opens no connection and no net touches another in more
//! places than before.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// The log with its colour codes taken out.
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
    shorts: usize,
    open_pins: usize,
}

/// `route` with the variants, which is where repair runs, and `check` on what it wrote.
fn route(fixture: &str, routed_name: &str) -> Routed {
    let dir = cypcb_fixtures::scratch_dir("cypcb-repair-open");
    let routed = dir.join(routed_name);
    let output = cypcb()
        .args(["route", "-o"])
        .arg(&routed)
        .arg(root().join(fixture))
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
    let violations = report["violations"]
        .as_array()
        .expect("a list of violations");
    Routed {
        said,
        shorts: violations
            .iter()
            .filter(|v| v["kind"] == "clearance" && v["actual_mm"] == 0.0)
            .count(),
        open_pins: violations
            .iter()
            .filter(|v| v["kind"] == "unrouted-pin")
            .count(),
    }
}

/// 7 shorts and 10 connections open before repair, 5 and 8 after. The winner
/// says it was repaired and still says how many connections it left open: a
/// repaired board with connections open is `Partial`, not `Complete`.
#[test]
fn esp32_starter_is_repaired_and_says_what_it_left_open() {
    let routed = route(
        "tests/fixtures/benchmark/esp32_starter.cypcb",
        "esp32.cypcb",
    );
    assert!(
        routed.said.contains(", repaired"),
        "the winner was not repaired:\n{}",
        routed.said
    );
    assert!(
        routed.said.contains("8 connection(s) could not be routed"),
        "the run does not say 8 connections are open:\n{}",
        routed.said
    );
    assert!(
        routed.said.contains("5 of them copper touching copper"),
        "{}",
        routed.said
    );
}

/// Repair kept an attempt on `mains-sequencer` only if no net touched another
/// more than before; every attempt traded connections for shorts, so none was
/// kept: 11 shorts and 41 connections open, as the first routing left them.
#[test]
fn mains_sequencer_keeps_its_shorts_and_open_connections_when_repair_would_trade_them() {
    let routed = route("examples/mains-sequencer.cypcb", "mains.cypcb");
    assert!(
        !routed.said.contains(", repaired"),
        "an attempt that trades was kept:\n{}",
        routed.said
    );
    assert!(
        routed.said.contains("41 connection(s) could not be routed"),
        "{}",
        routed.said
    );
    assert!(
        routed.said.contains("11 of them copper touching copper"),
        "{}",
        routed.said
    );
}

/// 22 shorts and 6 pins open before repair, 11 and 0 after: the attempt without
/// the strip closed every connection and touched no net more than before.
#[test]
fn stm32_breakout_has_half_the_shorts_and_no_pin_open() {
    let routed = route(
        "tests/fixtures/benchmark/stm32_breakout.kicad_pcb",
        "stm32.kicad_pcb",
    );
    assert_eq!(routed.shorts, 11, "{}", routed.said);
    assert_eq!(routed.open_pins, 0, "{}", routed.said);
}
