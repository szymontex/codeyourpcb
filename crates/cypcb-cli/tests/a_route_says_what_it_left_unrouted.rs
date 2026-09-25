//! `cypcb route` routes the parts drawn with a footprint the design defines,
//! and says so when it leaves a connection, on every road through it.
//!
//! `cargo test -p cypcb-cli --test a_route_says_what_it_left_unrouted`
//!
//! Two routing tests once routed with a library that lacked the design's own
//! footprints, and the router said Complete over nets it never saw. The
//! commands route with the library they synchronised with; this holds them to
//! it from the outside, on `examples/blink.cypcb`, whose LED is `LED_0805`.
//!
//! Until 2026-09-26 the default road - best of the variants - built its
//! result as Complete whatever the winner left, so the warning `--fast`
//! prints never came from the plain command. The walled board below leaves one
//! connection on both roads.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

/// The roads a `.cypcb` board takes through `cypcb route`.
const ROADS: [&[&str]; 2] = [&[], &["--fast"]];

const WARNING: &str = "could not be routed";

/// One net that cannot be routed - R2 sits inside a keepout on both layers -
/// and one that can, so the run has copper to write.
const WALLED: &str = r#"version 1

board walled {
    size 30mm x 20mm
    layers 2
}

keepout wall_top {
    bounds 15mm, 5mm to 25mm, 15mm
    layer top
}

keepout wall_bottom {
    bounds 15mm, 5mm to 25mm, 15mm
    layer bottom
}

component R1 resistor "0402" {
    at 5mm, 10mm
}

component R2 resistor "0402" {
    at 20mm, 10mm
}

component R3 resistor "0402" {
    at 5mm, 15mm
}

net N {
    R1.1
    R2.1
}

net M {
    R1.2
    R3.2
}
"#;

fn scratch(who: &str, name: &str, text: &str) -> (cypcb_fixtures::ScratchPath, PathBuf) {
    let dir = cypcb_fixtures::scratch_dir(&format!("cypcb-route-left-{who}"));
    let target = dir.join(name);
    std::fs::write(&target, text).expect("the scratch directory is writable");
    let routed = target.with_extension("routed.cypcb");
    (dir.holding(target), routed)
}

fn route(board: &Path, road: &[&str]) -> String {
    let output = cypcb()
        .arg("route")
        .args(road)
        .arg(board)
        .env("RUST_LOG", "off")
        .env_remove("FREEROUTING_JAR")
        .output()
        .expect("the binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(output.status.success(), "{road:?}:\n{stderr}");
    stderr
}

/// The pins `cypcb check` finds no copper on.
fn unrouted_pins(routed: &Path) -> Vec<String> {
    let output = cypcb()
        .arg("check")
        .arg(routed)
        .env("RUST_LOG", "off")
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .chain(String::from_utf8_lossy(&output.stderr).lines())
        .filter(|line| line.contains("unrouted-pin at"))
        .map(str::to_string)
        .collect()
}

#[test]
fn the_led_on_the_designs_own_footprint_is_routed_on_every_road() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/blink.cypcb");
    let text = std::fs::read_to_string(source).expect("the example is on disk");
    assert!(
        text.contains("footprint LED_0805 {") && text.contains("\"LED_0805\""),
        "the premise: blink draws its LED with a footprint it defines"
    );
    for (n, road) in ROADS.iter().enumerate() {
        let (board, routed) = scratch(&format!("blink-{n}"), "blink.cypcb", &text);
        let stderr = route(&board, road);
        let left = unrouted_pins(&routed);
        assert!(
            left.iter().all(|line| !line.contains("LED1.")),
            "{road:?} left the LED's pins: {left:#?}"
        );
        // What the router said and what the checker finds agree.
        assert_eq!(
            stderr.contains(WARNING),
            !left.is_empty(),
            "{road:?}: the router's word against the checker's {left:#?}\n{stderr}"
        );
    }
}

#[test]
fn a_connection_left_unrouted_is_said_on_every_road() {
    for (n, road) in ROADS.iter().enumerate() {
        let (board, routed) = scratch(&format!("walled-{n}"), "walled.cypcb", WALLED);
        let stderr = route(&board, road);
        assert!(
            stderr.contains(&format!("Warning: 1 connection(s) {WARNING}")),
            "{road:?} left R2 alone without a word:\n{stderr}"
        );
        let left = unrouted_pins(&routed);
        assert!(
            left.iter().any(|line| line.contains("R2.1")),
            "the control: the checker finds R2.1 bare after {road:?}: {left:#?}"
        );
    }
}
