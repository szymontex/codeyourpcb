//! Copper too close to a hole that is not plated.
//!
//! `cargo test -p cypcb-cli --test copper_too_close_to_a_screw_hole`
//!
//! Every other rule that walks pad copper walks past a mounting hole, because
//! a mounting hole has no copper. The courtyard rule stops a *part* being
//! placed on one; nothing stops a *trace* being drawn across one - the
//! autorouter will not do it, but a trace drawn by hand, a board imported from
//! KiCad and a zone poured over the hole all arrive without the router's
//! opinion.
//!
//! When it is missed the drill cuts the trace, so the net is open, and the
//! copper it exposes at the hole wall touches the screw: a metal standoff then
//! ties that net to the chassis.
//!
//! `MountingHoleClearanceRule` measures it with `min_edge_clearance`, because
//! that is what such a hole is - a board edge cut into the middle of the
//! board - and nothing ran the rule. It is the fifth of six the registry
//! census found.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A board with an M3 mounting hole at its middle and one trace across it.
///
/// `{Y}` is the line the trace runs on: through the hole, or clear of it.
const BOARD: &str = r#"version 1

board bracket {
    size 30mm x 20mm
    layers 2
    fab jlcpcb
}

component H1 generic "MOUNT-M3" {
    value "M3"
    at 15mm, 10mm
}

component J1 connector "PIN-HDR-1x2" {
    value "in"
    at 4mm, {Y}
    rotate 90
}

component J2 connector "PIN-HDR-1x2" {
    value "out"
    at 26mm, {Y}
    rotate 90
}

net SIG {
    J1.1
    J2.1
}

trace SIG {
    from J1.1
    to J2.1
    layer Top
    width 0.2mm
}
"#;

fn check(who: &str, y: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-screw-hole-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, BOARD.replace("{Y}", y)).expect("the fixture is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&board)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn a_trace_across_the_hole_is_reported_by_the_holes_own_name() {
    // The trace runs along the line the hole's centre is on: 3.2mm of drill
    // straight through it.
    let said = check("through", "10mm");

    assert!(
        said.contains("Copper too close to unplated hole H1"),
        "the report has to name the hole, or the reader is left looking at the \
         board outline:\n{said}"
    );
    assert!(
        said.contains("0.30mm required"),
        "and measure it against the fab's edge clearance, which is what this \
         hole is:\n{said}"
    );
    assert!(
        said.contains("The drill cuts this copper open and the screw touches what is left"),
        "and say what happens, because the fault is invisible on screen:\n{said}"
    );
}

#[test]
fn the_same_trace_routed_around_the_hole_is_quiet() {
    // 3.5mm above the centre: past the 1.6mm drill radius and the 0.3mm the
    // fab asks for.
    let said = check("around", "13.5mm");
    assert!(
        !said.contains("unplated hole"),
        "this copper is nowhere near the screw:\n{said}"
    );
}

#[test]
fn the_shipped_panel_mount_example_stays_quiet() {
    // Four mounting holes and copper on the same board, which is the case this
    // rule would report wrongly if it measured a hole against itself.
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", "examples/panel-mount.cypcb"])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        !said.contains("unplated hole"),
        "the example keeps its copper clear of its screws:\n{said}"
    );
}
