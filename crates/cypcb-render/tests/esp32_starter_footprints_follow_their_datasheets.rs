//! The four footprints of esp32_starter carry their datasheets' figures.
//!
//! `cargo test -p cypcb-render --test esp32_starter_footprints_follow_their_datasheets`
//!
//! The board's footprints are drawn from part documents, not from a library,
//! and the header of the board names the page of each. This test holds the
//! figures themselves: move one pad, drop one pad of the ground grid or
//! change a hole, and the part no longer matches the page it cites. Every
//! figure below is the one the board's header gives, in millimetres:
//!
//! - ESP32-S3-WROOM-1 datasheet v1.8, Figure 3-1 (p.10) and Figure 11-1
//!   (p.45) with its DXF;
//! - GCT USB4105 drawing, sheet 1, "Recommended PCB Layout";
//! - E-Switch TL3342, catalogue page 22, "P.C. Mounting";
//! - Wurth WR-PHD 61301211121, page 1/7, "Recommended Hole Pattern".

use std::path::Path;

use cypcb_render::{ComponentInfo, PadInfo, PcbEngine};

/// A figure in millimetres as nanometres.
fn nm(mm: f64) -> i64 {
    (mm * 1_000_000.0).round() as i64
}

fn close(got: i64, want_mm: f64) -> bool {
    (got - nm(want_mm)).abs() <= 1_000
}

fn source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/benchmark/esp32_starter.cypcb");
    std::fs::read_to_string(&path).expect("the fixture reads")
}

/// The parts and the kinds of every violation of a board as drawn.
fn load(source: &str) -> (Vec<ComponentInfo>, Vec<String>) {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(source);
    assert!(errors.is_empty(), "the board loads: {errors}");
    let snapshot = engine.build_snapshot();
    let kinds = snapshot.violations.iter().map(|v| v.kind.clone()).collect();
    (snapshot.components, kinds)
}

fn board() -> (Vec<ComponentInfo>, Vec<String>) {
    load(&source())
}

fn part<'a>(components: &'a [ComponentInfo], refdes: &str) -> &'a ComponentInfo {
    components
        .iter()
        .find(|c| c.refdes == refdes)
        .unwrap_or_else(|| panic!("the board has no {refdes}"))
}

fn pads<'a>(part: &'a ComponentInfo, number: &str) -> Vec<&'a PadInfo> {
    part.pads.iter().filter(|p| p.number == number).collect()
}

fn the_pad<'a>(part: &'a ComponentInfo, number: &str) -> &'a PadInfo {
    let found = pads(part, number);
    assert_eq!(
        found.len(),
        1,
        "{} pad {number}: one pad expected",
        part.refdes
    );
    found[0]
}

#[test]
fn the_module_is_its_land_pattern() {
    let (components, _) = board();
    let u1 = part(&components, "U1");
    // Figure 11-1: 14 pads a side on columns 17.5 apart, 1.27 pitch, the
    // first 7.49 below the 25.5 long module's top edge; 12 pads along the
    // bottom, 0.9 x 1.5, the row 0.25 above the bottom edge.
    for i in 0..14usize {
        let y = 5.26 - 1.27 * i as f64;
        let left = the_pad(u1, &(1 + i).to_string());
        let right = the_pad(u1, &(40 - i).to_string());
        assert!(
            close(left.x_nm, -8.75) && close(left.y_nm, y),
            "pad {}",
            1 + i
        );
        assert!(
            close(right.x_nm, 8.75) && close(right.y_nm, y),
            "pad {}",
            40 - i
        );
        for p in [left, right] {
            assert!(
                close(p.width_nm, 1.5) && close(p.height_nm, 0.9),
                "pad {}",
                p.number
            );
        }
    }
    for i in 0..12usize {
        let p = the_pad(u1, &(15 + i).to_string());
        assert!(close(p.x_nm, -6.985 + 1.27 * i as f64), "pad {}", p.number);
        assert!(close(p.y_nm, -12.5), "pad {}", p.number);
        assert!(
            close(p.width_nm, 0.9) && close(p.height_nm, 1.5),
            "pad {}",
            p.number
        );
    }
    // Figure 3-1 and 11-1: pin 41 is a grid of nine 0.9 x 0.9 pads, 1.4 apart,
    // 3.7 over the outside.
    let grid = pads(u1, "41");
    assert_eq!(grid.len(), 9, "the ground grid has nine pads");
    for p in &grid {
        assert!(close(p.width_nm, 0.9) && close(p.height_nm, 0.9));
    }
    let span = |f: fn(&&PadInfo) -> i64| {
        grid.iter().map(f).max().unwrap() - grid.iter().map(f).min().unwrap()
    };
    assert!(close(span(|p| p.x_nm) + nm(0.9), 3.7), "grid width");
    assert!(close(span(|p| p.y_nm) + nm(0.9), 3.7), "grid height");
    // The grid sits 7.5 from the module's left edge (centre -9 + 7.5) and
    // 10.29 from its bottom edge (centre -12.75 + 10.29), Figure 11-1.
    let centre = |f: fn(&&PadInfo) -> i64| {
        (grid.iter().map(f).max().unwrap() + grid.iter().map(f).min().unwrap()) / 2
    };
    assert!(close(centre(|p| p.x_nm), -1.5), "grid centre x");
    assert!(close(centre(|p| p.y_nm), -2.46), "grid centre y");
    assert_eq!(u1.pads.len(), 49, "40 edge pads and the nine of the grid");
}

#[test]
fn the_receptacle_is_its_recommended_layout() {
    let (components, _) = board();
    let j1 = part(&components, "J1");
    // Contacts: 12 pads 1.15 long, the origin on the PCB edge line, centres
    // 7.355 from it (2.60 + 4.18 + half of 1.15). The pins that share a land
    // are two pads in one place.
    let land = |pin: &str| the_pad(j1, pin);
    for (pins, x, w) in [
        (["A1", "B12"], -3.2, 0.6),
        (["A4", "B9"], -2.4, 0.6),
        (["B4", "A9"], 2.4, 0.6),
        (["B1", "A12"], 3.2, 0.6),
    ] {
        for pin in pins {
            let p = land(pin);
            assert!(close(p.x_nm, x) && close(p.y_nm, 7.355), "pin {pin}");
            assert!(
                close(p.width_nm, w) && close(p.height_nm, 1.15),
                "pin {pin}"
            );
        }
    }
    for (pin, x) in [
        ("B8", -1.75),
        ("A5", -1.25),
        ("B7", -0.75),
        ("A6", -0.25),
        ("A7", 0.25),
        ("B6", 0.75),
        ("A8", 1.25),
        ("B5", 1.75),
    ] {
        let p = land(pin);
        assert!(close(p.x_nm, x) && close(p.y_nm, 7.355), "pin {pin}");
        assert!(
            close(p.width_nm, 0.3) && close(p.height_nm, 1.15),
            "pin {pin}"
        );
    }
    // Pegs: 2 x diameter 0.65, 5.78 apart, 3.68 above the lower slots.
    let pegs = pads(j1, "NPTH");
    assert_eq!(pegs.len(), 2);
    for p in pegs {
        assert!(close(p.x_nm.abs(), 2.89) && close(p.y_nm, 2.6 + 3.68));
        assert_eq!(p.drill_nm, Some(nm(0.65)));
    }
    // Shell: slots on 8.64 centres, the upper 2.10 x 1.00 with a 0.60 x 1.70
    // hole 4.18 above the lower, 1.80 x 1.00 with a 0.60 x 1.40 hole, 2.60
    // from the edge line.
    let shell = pads(j1, "SH");
    assert_eq!(shell.len(), 4);
    for p in shell {
        assert!(close(p.x_nm.abs(), 4.32), "slot x");
        let upper = close(p.y_nm, 6.78);
        let lower = close(p.y_nm, 2.6);
        assert!(upper || lower, "slot y {}", p.y_nm);
        let (h, hole) = if upper { (2.1, 1.7) } else { (1.8, 1.4) };
        assert!(close(p.width_nm, 1.0) && close(p.height_nm, h));
        assert_eq!(p.drill_nm, Some(nm(0.6)));
        assert_eq!(p.slot_nm.map(|s| s.1), Some(nm(hole)));
    }
    assert_eq!(j1.pads.len(), 22);
}

#[test]
fn the_tact_switch_is_its_pc_mounting() {
    let (components, _) = board();
    for refdes in ["SW1", "SW2"] {
        let sw = part(&components, refdes);
        assert_eq!(sw.pads.len(), 4, "{refdes}: four terminals");
        // 8.00 over the outside, 4.60 over the inside: pads 1.70 wide on
        // 3.15. 4.80 outside to outside, 2.80 inside: pads 1.00 on 1.90.
        for p in &sw.pads {
            assert!(
                close(p.x_nm.abs(), 3.15) && close(p.y_nm.abs(), 1.9),
                "{refdes}"
            );
            assert!(
                close(p.width_nm, 1.7) && close(p.height_nm, 1.0),
                "{refdes}"
            );
        }
        // Pad 1 is the upper pair (terminals 3 and 4), pad 2 the lower.
        for (number, y) in [("1", 1.9), ("2", -1.9)] {
            let pair = pads(sw, number);
            assert_eq!(pair.len(), 2, "{refdes} pad {number}");
            assert!(
                pair.iter().all(|p| close(p.y_nm, y)),
                "{refdes} pad {number}"
            );
        }
    }
}

#[test]
fn the_pin_header_is_its_hole_pattern() {
    let (components, _) = board();
    for refdes in ["J2", "J3"] {
        let hdr = part(&components, refdes);
        assert_eq!(hdr.pads.len(), 12);
        for i in 0..12usize {
            let p = the_pad(hdr, &(i + 1).to_string());
            assert!(
                close(p.x_nm, 0.0) && close(p.y_nm, -2.54 * i as f64),
                "{refdes} pad {}",
                p.number
            );
            // Wurth 61301211121: hole 1.10 on a 2.54 pitch, 27.94 pin to pin.
            assert_eq!(p.drill_nm, Some(nm(1.1)), "{refdes} pad {}", p.number);
            assert!(close(p.width_nm, 1.7) && close(p.height_nm, 1.7));
        }
    }
}

fn count(kinds: &[String], kind: &str) -> usize {
    kinds.iter().filter(|k| *k == kind).count()
}

#[test]
fn no_courtyard_misses_its_pads_or_touches_its_neighbour() {
    // Every courtyard holds its part's body and pads with 0.25 over, so the
    // two courtyard rules have nothing to say about the board as drawn.
    let (_, kinds) = board();
    for kind in ["land-outside-courtyard", "courtyard-clearance"] {
        let n = count(&kinds, kind);
        assert_eq!(n, 0, "{n} {kind} rows on the starter board");
    }
}

#[test]
fn control_the_two_courtyard_rules_speak_when_a_courtyard_is_wrong() {
    // The header's courtyard as the board had it before it was redrawn,
    // 0.5 over the pin pitch on each side: D2 stood 0.19 from J3's.
    let wide = source().replace(
        "courtyard 3.04mm x 30.98mm at 0mm, -13.97mm",
        "courtyard 3.54mm x 31.48mm at 0mm, -13.97mm",
    );
    assert_ne!(
        wide,
        source(),
        "the header's courtyard line is in the board"
    );
    let (_, kinds) = load(&wide);
    assert!(
        count(&kinds, "courtyard-clearance") >= 1,
        "the wide courtyard touches D2"
    );

    // A courtyard too small for the pads.
    let small = source().replace(
        "courtyard 3.04mm x 30.98mm at 0mm, -13.97mm",
        "courtyard 1mm x 1mm",
    );
    let (_, kinds) = load(&small);
    assert!(
        count(&kinds, "land-outside-courtyard") >= 1,
        "pads outside a 1mm courtyard"
    );
}
