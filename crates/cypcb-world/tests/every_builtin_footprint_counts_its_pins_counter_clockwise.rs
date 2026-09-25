//! Every built-in footprint has pin 1 where the part has it and counts its
//! pins the way the part does.
//!
//! `cargo test -p cypcb-world --test every_builtin_footprint_counts_its_pins_counter_clockwise`
//!
//! The convention is the one `cypcb_world::footprint` states: Y grows up,
//! seen from the top, pin 1 at the top left and the pins counting
//! counter-clockwise - IPC-7351's zero orientation, as rule F4.2 of KiCad's
//! library conventions writes it. Until 2026-09-25 five of the thirteen
//! built-ins with pins broke it and nothing noticed, because every test asked where a
//! pin was on its own and none asked which way the pins ran:
//!
//! - SOT-23-5 was the part's mirror image, pin 1 at the bottom left, so a
//!   regulator placed from it was soldered with its input on its output;
//! - SOIC-8 and SOIC-14 ran both columns upwards, so pin 8 sat opposite pin 4
//!   instead of pin 1 - no real package, mirrored or not;
//! - SOT-23 and TQFP-32 were the part turned a quarter, which is the part a
//!   pick-and-place file at rotation 0 puts down turned a quarter.
//!
//! Two properties, both read from the pads alone:
//!
//! 1. A part whose pins are not in one line: the pins, joined in number
//!    order, enclose a positive area - they run counter-clockwise - and pin 1
//!    is in the top-left quadrant.
//! 2. A part whose pins are in one line: pin 1 is at the left end of a
//!    horizontal line.
//!
//! A mirror image fails 1 on the sign of the area, a column running the wrong
//! way fails it on the area collapsing, a quarter turn fails the quadrant.
//! A footprint with no numbered pin - a mounting hole - has no pin 1 to put
//! anywhere and is listed below by name, so a new footprint cannot slip past
//! by being unlisted.

use cypcb_world::footprint::{Footprint, FootprintLibrary};

/// Footprints with no pin order to check, and why.
const NO_PIN_ORDER: [(&str, &str); 4] = [
    ("MOUNT-M2", "one unnumbered hole"),
    ("MOUNT-M2.5", "one unnumbered hole"),
    ("MOUNT-M3", "one unnumbered hole"),
    ("MOUNT-M4", "one unnumbered hole"),
];

/// The numbered pads in pin order, as (x, y) in millimetres.
fn pins(footprint: &Footprint) -> Vec<(f64, f64)> {
    let mut numbered: Vec<(u32, (f64, f64))> = footprint
        .pads
        .iter()
        .filter_map(|pad| {
            let number = pad.number.parse().ok()?;
            Some((number, (pad.position.x.to_mm(), pad.position.y.to_mm())))
        })
        .collect();
    numbered.sort_by_key(|(number, _)| *number);
    numbered.into_iter().map(|(_, at)| at).collect()
}

/// Twice the signed area the pins enclose, joined in order: positive when
/// they run counter-clockwise in a Y-up frame.
fn winding(pins: &[(f64, f64)]) -> f64 {
    (0..pins.len())
        .map(|i| {
            let (x0, y0) = pins[i];
            let (x1, y1) = pins[(i + 1) % pins.len()];
            x0 * y1 - x1 * y0
        })
        .sum()
}

/// What is wrong with this footprint's pin order, if anything.
fn fault(footprint: &Footprint) -> Option<String> {
    let pins = pins(footprint);
    let (x1, y1) = pins[0];
    let in_one_line = pins.iter().all(|&(_, y)| (y - y1).abs() < 1e-6)
        || pins.iter().all(|&(x, _)| (x - x1).abs() < 1e-6);
    if in_one_line {
        let horizontal = pins.iter().all(|&(_, y)| (y - y1).abs() < 1e-6);
        let leftmost = pins.iter().all(|&(x, _)| x1 <= x);
        return (!(horizontal && leftmost))
            .then(|| format!("pin 1 at ({x1}, {y1}) is not the left end of a horizontal row"));
    }
    let area = winding(&pins);
    if area <= 1e-6 {
        return Some(format!(
            "the pins run clockwise or cross over (twice the area {area:.3} mm2)"
        ));
    }
    let (cx, cy) = pins
        .iter()
        .fold((0.0, 0.0), |(sx, sy), &(x, y)| (sx + x, sy + y));
    let (cx, cy) = (cx / pins.len() as f64, cy / pins.len() as f64);
    (!(x1 < cx && y1 > cy))
        .then(|| format!("pin 1 at ({x1}, {y1}) is not in the top-left quadrant"))
}

#[test]
fn every_builtin_footprint_counts_its_pins_counter_clockwise() {
    let library = FootprintLibrary::new();
    let mut faults = Vec::new();
    let mut checked = 0;
    for (name, footprint) in library.iter() {
        if NO_PIN_ORDER.iter().any(|(listed, _)| *listed == name) {
            assert!(
                pins(footprint).is_empty(),
                "{name} is listed as having no pin order but has numbered pins"
            );
            continue;
        }
        assert!(
            pins(footprint).len() >= 2,
            "{name} has no pin order to check: list it in NO_PIN_ORDER with the reason"
        );
        checked += 1;
        if let Some(fault) = fault(footprint) {
            faults.push(format!("{name}: {fault}"));
        }
    }
    assert!(faults.is_empty(), "{}", faults.join("\n"));
    assert_eq!(
        checked + NO_PIN_ORDER.len(),
        library.len(),
        "every built-in is either checked or listed"
    );
}

#[test]
fn a_mirrored_part_is_caught() {
    // The property has to be able to fail: SOT-23-5 flipped top to bottom is
    // exactly what the library held until 2026-09-25.
    let library = FootprintLibrary::new();
    let mut mirrored = library.get("SOT-23-5").expect("built in").clone();
    for pad in &mut mirrored.pads {
        pad.position.y = -pad.position.y;
    }
    assert!(fault(&mirrored).is_some());
}
