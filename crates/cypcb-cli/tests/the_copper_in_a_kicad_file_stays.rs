//! The copper a KiCad file carries stays where it is, and the router goes round it.
//!
//! `cargo test --release -p cypcb-cli --test the_copper_in_a_kicad_file_stays`
//!
//! Until 2026-10-31 `cypcb route` on a `.kicad_pcb` routed as if the file held
//! no copper. The copy it wrote carried the router's traces on top of the
//! designer's: on 92 of the 132 benchmark boards the file holds traces or vias,
//! and `route --fast` left 23485 copper-on-copper contacts on them, 7155 since.
//! `usb_joystick_routed` is the small one: 7 contacts blind, 4 with the
//! file's copper in the way. A board the file has already joined needs no
//! routing, and used to come back with a second set of traces on top.
//!
//! `--ignore-board-copper` asks for the old board: the parts alone.

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

struct Run {
    success: bool,
    said: String,
    /// Holds the scratch directory until the test is done with the file.
    routed: cypcb_fixtures::ScratchPath,
}

fn route(fixture: &Path, extra: &[&str], who: &str) -> Run {
    let dir = cypcb_fixtures::scratch_dir(&format!("cypcb-file-copper-{who}"));
    let routed = dir.join("routed.kicad_pcb");
    let output = cypcb()
        .args(["route", "--fast"])
        .args(extra)
        .arg("-o")
        .arg(&routed)
        .arg(fixture)
        .output()
        .expect("the binary runs");
    Run {
        success: output.status.success(),
        said: plain(&format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )),
        routed: dir.holding(routed),
    }
}

/// Copper touching copper, as `check` counts it on the routed file.
fn contacts(board: &Path) -> usize {
    let check = cypcb()
        .args(["check", "-o", "json"])
        .arg(board)
        .output()
        .expect("the binary runs");
    let report: serde_json::Value =
        serde_json::from_slice(&check.stdout).expect("check prints JSON");
    report["violations"]
        .as_array()
        .expect("a list of violations")
        .iter()
        .filter(|v| v["kind"] == "clearance" && v["actual_mm"] == 0.0)
        .count()
}

fn usb_joystick() -> PathBuf {
    root().join("tests/fixtures/kicad-tools/boards/03-usb-joystick/usb_joystick_routed.kicad_pcb")
}

fn joined_board() -> PathBuf {
    root().join("crates/cypcb-kicad/tests/fixtures/kicad10-slotted.kicad_pcb")
}

#[test]
fn the_router_goes_round_the_copper_the_file_carries() {
    let run = route(&usb_joystick(), &[], "round");
    assert!(run.success, "{}", run.said);
    assert_eq!(contacts(&run.routed), 4, "{}", run.said);
}

#[test]
fn a_board_asked_for_without_its_copper_is_routed_as_it_was() {
    let run = route(&usb_joystick(), &["--ignore-board-copper"], "blind");
    assert!(run.success, "{}", run.said);
    assert_eq!(contacts(&run.routed), 7, "{}", run.said);
}

/// Every connection of `kicad10-slotted` is joined by the two traces in its
/// file. The run says so, succeeds, and the output is the input.
#[test]
fn a_board_the_file_has_joined_comes_back_as_it_went_in() {
    let run = route(&joined_board(), &[], "joined");
    assert!(
        run.success,
        "a joined board is not a failure:\n{}",
        run.said
    );
    assert!(
        run.said.contains(
            "nothing to route: every connection is already joined by the copper in the file"
        ),
        "{}",
        run.said
    );
    let input = std::fs::read(joined_board()).expect("the fixture reads");
    let output = std::fs::read(&*run.routed).expect("the copy is on disk");
    assert!(input == output, "the output is not a copy of the input");
}

/// Asked to ignore the file's copper, the same board has connections to lay.
#[test]
fn a_joined_board_asked_for_without_its_copper_is_routed() {
    let run = route(&joined_board(), &["--ignore-board-copper"], "joined-blind");
    assert!(run.success, "{}", run.said);
    assert!(!run.said.contains("nothing to route"), "{}", run.said);
    let input = std::fs::read(joined_board()).expect("the fixture reads");
    let output = std::fs::read(&*run.routed).expect("the routed board is on disk");
    assert!(input != output, "no copper was laid");
}

#[test]
fn a_design_has_no_copper_to_ignore() {
    let design = root().join("tests/fixtures/benchmark/esp32_starter.cypcb");
    let run = route(&design, &["--ignore-board-copper"], "design");
    assert!(!run.success, "{}", run.said);
    assert!(
        run.said
            .contains("--ignore-board-copper applies to a .kicad_pcb board"),
        "{}",
        run.said
    );
}
