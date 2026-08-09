//! Which part goes on the pads.
//!
//! `cargo test -p cypcb-cli --test the_part_to_buy_reaches_the_bom`
//!
//! The viewer has fetched footprints from LCSC for a long time, and the way a
//! design asked for one was `lcsc "C7593"` inside a component - which the
//! grammar did not have. The reader dropped it in silence, so the browser
//! read it out of the raw text with a regular expression while the board model
//! never saw it, and on 2026-08-09, when unknown properties became an error,
//! the same file stopped checking at all.
//!
//! It is a property of the language now, and it goes where it is useful: a
//! bill of materials without a part number says how many of something to buy
//! and not which something.

use std::process::Command;

const BOARD: &str = r#"version 1

board parts {
    size 30mm x 20mm
    layers 2
}

component U1 ic "SOIC-8" {
    value "NE555"
    lcsc "C7593"
    at 10mm, 10mm
}

component R1 resistor "0402" {
    value "10k"
    lcsc "C25804"
    at 20mm, 10mm
}

// Same value, same footprint, a different catalogue part: two lines to order.
component R2 resistor "0402" {
    value "10k"
    lcsc "C17414"
    at 25mm, 10mm
}

// And one the design does not name a part for.
component R3 resistor "0402" {
    value "10k"
    at 5mm, 5mm
}
"#;

fn run(args: &[&str], file: &std::path::Path) -> (String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .arg(file)
        .output()
        .expect("the binary runs");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn board_file() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("cypcb-lcsc");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let file = dir.join("parts.cypcb");
    std::fs::write(&file, BOARD).expect("the board is written");
    file
}

#[test]
fn a_design_that_names_its_parts_is_accepted() {
    let (_, errors) = run(&["check"], &board_file());

    assert!(
        !errors.contains("has no property"),
        "`lcsc` is a property of the language: {errors}"
    );
}

#[test]
fn the_bom_says_which_part_to_buy() {
    let file = board_file();
    let out = std::env::temp_dir().join("cypcb-lcsc-out");
    let _ = std::fs::remove_dir_all(&out);
    run(&["export", "-o", out.to_str().unwrap()], &file);

    let bom = std::fs::read_to_string(out.join("assembly/parts-BOM.csv"))
        .expect("the exporter wrote a bill of materials");

    assert!(
        bom.lines().next().unwrap().contains("LCSC Part #"),
        "the column an assembly house fills in has to be there:\n{bom}"
    );
    assert!(
        bom.contains("C7593"),
        "the part the design names has to be in it:\n{bom}"
    );
}

#[test]
fn two_catalogue_parts_are_two_lines_even_when_they_look_alike() {
    // R1 and R2 are both 10k 0402s and are different things to order. Grouping
    // them together would have somebody buy twice as many of the wrong one.
    let file = board_file();
    let out = std::env::temp_dir().join("cypcb-lcsc-out2");
    let _ = std::fs::remove_dir_all(&out);
    run(&["export", "-o", out.to_str().unwrap()], &file);

    let bom = std::fs::read_to_string(out.join("assembly/parts-BOM.csv"))
        .expect("the exporter wrote a bill of materials");

    let lines: Vec<&str> = bom.lines().filter(|l| l.contains("0402")).collect();
    assert_eq!(
        lines.len(),
        3,
        "C25804, C17414 and the one with no part number are three lines:\n{bom}"
    );

    // And the one the design says nothing about leaves the column empty rather
    // than borrowing a neighbour's part number.
    assert!(
        lines.iter().any(|l| l.trim_end().ends_with(',')),
        "R3 names no part, so its column is empty:\n{bom}"
    );
}
