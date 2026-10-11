//! `--teardrops` asks before the board is checked, so the checker reads the
//! copper the Gerber files will carry.
//!
//! `cargo test -p cypcb-cli --test the_flag_asks_before_the_check`
//!
//! The flag used to reach the Gerber writer alone. The fillet was drawn into
//! the files after the check had passed the board, so a fillet that came too
//! close to another net's copper went to the fabricator unreported. The flag
//! now sets the request in the board, ahead of the check.
//!
//! The board: a round land of net A with a track leaving it, and a small land
//! of net B just beside the track's flank. Without the fillet every gap is
//! wide enough; with it the flank is inside the fab's distance of B's land.

use std::process::Command;

const BOARD: &str = r#"version 1

footprint LAND {
    description "one round land"
    courtyard 2.4mm x 2.4mm
    pad 1 circle at 0mm, 0mm size 1.6mm x 1.6mm
}

footprint DOT {
    description "one small land"
    courtyard 0.8mm x 0.8mm
    pad 1 circle at 0mm, 0mm size 0.3mm x 0.3mm
}

board flag {
    size 30mm x 20mm
    layers 2
    fab jlcpcb
}

component A1 resistor "LAND" {
    value "a"
    at 10mm, 10mm
}

component A2 resistor "LAND" {
    value "a"
    at 20mm, 10mm
}

component B1 resistor "DOT" {
    value "b"
    at 11mm, 10.62mm
}

component B2 resistor "DOT" {
    value "b"
    at 13mm, 10.62mm
}

net A {
    A1.1
    A2.1
}

net B {
    B1.1
    B2.1
}

trace A {
    from A1.1
    to A2.1
    layer Top
    width 0.2mm
}

trace B {
    from B1.1
    to B2.1
    layer Top
    width 0.2mm
}
"#;

fn exported(flag: bool, who: &str) -> (bool, String) {
    let dir_home = cypcb_fixtures::scratch_dir(&format!("cypcb-flag-{who}"));
    let board = dir_home.join("flag.cypcb");
    std::fs::write(&board, BOARD).expect("the board is written");
    let out = dir_home.join("out");
    let mut command = Command::new(env!("CARGO_BIN_EXE_cypcb"));
    command.env("SOURCE_DATE_EPOCH", "0");
    command.arg("export").arg(&board).arg("-o").arg(&out);
    if flag {
        command.arg("--teardrops");
    }
    let output = command.output().expect("the binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stderr)
}

fn violations_said(stderr: &str) -> usize {
    stderr
        .split("exporting a board with ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

#[test]
fn the_flag_puts_the_fillet_in_front_of_the_checker() {
    let (ok_without, without) = exported(false, "without");
    assert!(ok_without, "the board exports without the flag:\n{without}");
    let (ok_with, with) = exported(true, "with");
    assert!(ok_with, "and with it:\n{with}");

    // The fillet of A is close to B's land, B's track and B's own fillet:
    // three pairs, three rows, none of them there without it.
    assert_eq!(
        violations_said(&with),
        violations_said(&without) + 3,
        "the fillets add the rows the checker reads from them.\nwithout:\n{without}\nwith:\n{with}"
    );
}
