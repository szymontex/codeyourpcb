//! A board handed to KiCad is on the paper, not jammed into its corner.
//!
//! `cargo test -p cypcb-kicad --test the_board_sits_on_the_sheet`
//!
//! A design counts from its own corner: a 40x25mm board runs from 0,0 to
//! 40,25. Those numbers went into the file unchanged, and the file says
//! `(paper "A4")` - 297x210mm. So every board this project ever wrote opened
//! in pcbnew squeezed into the top-left corner of the sheet, with the whole
//! rest of the paper empty beside it.
//!
//! Nothing refused the file for it. KiCad drew it in the corner and said
//! nothing, which is why this went unseen until somebody opened one.
//!
//! What must not change while fixing it: the board itself. Moving every
//! coordinate by the same offset moves the drawing, not the design, and the
//! last test here is the one that says so.

use cypcb_core::Nm;
use cypcb_kicad::board_writer::write_board;
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation, Value};
use cypcb_world::BoardWorld;

/// Two parts 15mm apart on a 40x25mm board.
fn board(width_mm: f64, height_mm: f64) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "on_the_sheet".to_string(),
        (Nm::from_mm(width_mm), Nm::from_mm(height_mm)),
        2,
    );
    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(10.0, 12.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );
    world.spawn_component(
        RefDes::new("R2"),
        Value::new("10k"),
        Position::from_mm(25.0, 12.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );
    world
}

/// Every number written on an absolute coordinate, x and y interleaved.
fn coordinates(text: &str) -> Vec<(f64, f64)> {
    let mut found = Vec::new();
    for line in text.lines() {
        let keys = ["(start ", "(end ", "(at ", "(xy "];
        // Pads and footprint graphics are relative to their footprint's own
        // placement, so only the top-level lines carry sheet coordinates.
        if line.starts_with("    ") {
            continue;
        }
        for key in keys {
            let mut rest = line;
            while let Some(index) = rest.find(key) {
                rest = &rest[index + key.len()..];
                let mut numbers = rest.split_whitespace();
                let x = numbers
                    .next()
                    .and_then(|n| n.trim_end_matches(')').parse().ok());
                let y = numbers
                    .next()
                    .and_then(|n| n.trim_end_matches(')').parse().ok());
                if let (Some(x), Some(y)) = (x, y) {
                    found.push((x, y));
                }
            }
        }
    }
    found
}

#[test]
fn the_board_is_centred_on_the_sheet() {
    let text = write_board(&mut board(40.0, 25.0), "cypcb");
    let outline: Vec<_> = text
        .lines()
        .filter(|line| line.contains("Edge.Cuts"))
        .flat_map(coordinates)
        .collect();
    assert!(!outline.is_empty(), "the board has no outline:\n{text}");

    let left = outline.iter().map(|(x, _)| *x).fold(f64::MAX, f64::min);
    let right = outline.iter().map(|(x, _)| *x).fold(f64::MIN, f64::max);
    let top = outline.iter().map(|(_, y)| *y).fold(f64::MAX, f64::min);
    let bottom = outline.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);

    // A4 is 297x210mm, so its middle is 148.5, 105.
    assert!(
        ((left + right) / 2.0 - 148.5).abs() < 0.001,
        "the board is at x {left}..{right}, and the sheet's middle is 148.5"
    );
    assert!(
        ((top + bottom) / 2.0 - 105.0).abs() < 0.001,
        "the board is at y {top}..{bottom}, and the sheet's middle is 105"
    );
    assert!(
        (right - left - 40.0).abs() < 0.001 && (bottom - top - 25.0).abs() < 0.001,
        "the board changed size: {}x{}",
        right - left,
        bottom - top
    );
}

#[test]
fn a_board_larger_than_the_sheet_keeps_a_margin() {
    // Centring a 400mm board on a 297mm sheet would put its left edge at -51.5
    // and swap one problem for the other.
    let text = write_board(&mut board(400.0, 300.0), "cypcb");
    let off: Vec<_> = coordinates(&text)
        .into_iter()
        .filter(|(x, y)| *x < 0.0 || *y < 0.0)
        .collect();

    assert!(off.is_empty(), "a big board went negative: {off:?}");
}

#[test]
fn the_design_is_not_changed_by_being_placed() {
    // The whole board moves, so everything on it keeps its distance to
    // everything else. R1 and R2 are 15mm apart in the design.
    let text = write_board(&mut board(40.0, 25.0), "cypcb");
    let parts: Vec<(f64, f64)> = text
        .lines()
        .filter(|line| line.starts_with("  (footprint "))
        .flat_map(coordinates)
        .collect();

    assert_eq!(parts.len(), 2, "expected two parts, got {parts:?}\n{text}");
    let gap = (parts[0].0 - parts[1].0).abs();
    assert!(
        (gap - 15.0).abs() < 0.001,
        "the parts are {gap}mm apart and the design puts them 15mm apart"
    );
    assert!(
        (parts[0].1 - parts[1].1).abs() < 0.001,
        "the parts left the same row: {parts:?}"
    );
}
