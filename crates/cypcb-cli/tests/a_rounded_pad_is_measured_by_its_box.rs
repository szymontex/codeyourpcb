//! A rounded pad is measured by the box it sits in, and what that costs.
//!
//! `cargo test -p cypcb-cli --test a_rounded_pad_is_measured_by_its_box`
//!
//! `ClearanceRule` measures pads as axis-aligned boxes: two pads meeting
//! corner to corner are `sqrt(dx^2 + dy^2)` apart, between their sharp
//! corners. A `roundrect` pad has no sharp corner - the arc pulls the nearest
//! copper back along the diagonal by `r * (sqrt(2) - 1)` - so the copper is
//! further apart than the checker thinks. The error is one-directional: the
//! checker can refuse a board that is fine, and cannot pass one that is not.
//!
//! The question this file answers is how much, measured rather than reasoned
//! about, because the language learned to state a corner and nothing read it.
//!
//! Two answers, and they point different ways:
//!
//! - the correction is **larger than the clearance it is measured against** -
//!   207 microns on `charlieplex_3x3`, against JLCPCB's 127 micron minimum
//! - and it changes **no verdict on any board in this repository**: not one
//!   diagonal pad pair sits inside the limit by its boxes and outside it by
//!   its copper
//!
//! So the rule is left alone and this is the guard that catches the day that
//! stops being true. A board where the two disagree fails the second case
//! here, and then the rule is worth teaching about corners.

use cypcb_core::Nm;
use cypcb_drc::rules::ClearanceRule;
use cypcb_drc::{DesignRules, DrcRule};
use cypcb_world::components::{FootprintRef, Layer, PadShape, Position, Rotation};
use std::path::{Path, PathBuf};

/// JLCPCB's minimum copper clearance, which is what the boards below are
/// checked against.
const MIN_CLEARANCE_NM: f64 = 127_000.0;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two directories above this crate")
}

/// Every KiCad board in this repository that this reader can read.
const BOARDS: [&str; 4] = [
    "viewer/kicad-tools/boards/03-usb-joystick/output/usb_joystick_routed.kicad_pcb",
    "viewer/kicad-tools/boards/01-voltage-divider/output/voltage_divider.kicad_pcb",
    "viewer/kicad-tools/boards/02-charlieplex-led/output/charlieplex_3x3.kicad_pcb",
    "viewer/kicad-tools/tests/fixtures/routing-diagnostic.kicad_pcb",
];

/// One pad in board coordinates: centre, half-size, and the radius its corners
/// are drawn with.
struct Placed {
    x: f64,
    y: f64,
    half_width: f64,
    half_height: f64,
    radius: f64,
}

fn pads_of(board: &str) -> Vec<Placed> {
    let parsed = cypcb_kicad::parse_kicad_pcb(&repo_root().join(board))
        .unwrap_or_else(|error| panic!("{board}: {error}"));
    let mut world = parsed.world;
    let library = world.footprints().clone();

    let parts: Vec<(FootprintRef, Position, Rotation)> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(&FootprintRef, &Position, &Rotation)>();
        query
            .iter(ecs)
            .map(|(f, p, r)| (f.clone(), *p, *r))
            .collect()
    };

    let mut placed = Vec::new();
    for (footprint_ref, position, rotation) in parts {
        let Some(footprint) = library.get(footprint_ref.as_str()) else {
            continue;
        };
        let degrees = rotation.to_degrees();
        let (sin, cos) = degrees.to_radians().sin_cos();
        for pad in &footprint.pads {
            if !pad.layers.contains(&Layer::TopCopper) {
                continue;
            }
            let (px, py) = (pad.position.x.0 as f64, pad.position.y.0 as f64);
            let quarter = ((degrees / 90.0).round() * 90.0 - degrees).abs() < 1e-6
                && ((degrees / 180.0).round() * 180.0 - degrees).abs() > 1e-6;
            let (width, height) = if quarter {
                (pad.size.1 .0 as f64, pad.size.0 .0 as f64)
            } else {
                (pad.size.0 .0 as f64, pad.size.1 .0 as f64)
            };
            let radius = match pad.shape {
                PadShape::RoundRect { corner_ratio } => {
                    width.min(height) * (corner_ratio as f64) / 100.0
                }
                _ => 0.0,
            };
            placed.push(Placed {
                x: position.0.x.0 as f64 + px * cos - py * sin,
                y: position.0.y.0 as f64 + px * sin + py * cos,
                half_width: width / 2.0,
                half_height: height / 2.0,
                radius,
            });
        }
    }
    placed
}

/// The largest correction a rounded corner makes on this board, and how many
/// pad pairs change side of the clearance limit because of it.
fn corner_effect(board: &str) -> (f64, usize) {
    let pads = pads_of(board);
    let mut worst = 0.0f64;
    let mut flipped = 0;
    for (i, a) in pads.iter().enumerate() {
        for b in &pads[i + 1..] {
            let dx = ((b.x - a.x).abs() - a.half_width - b.half_width).max(0.0);
            let dy = ((b.y - a.y).abs() - a.half_height - b.half_height).max(0.0);
            // Only a corner-to-corner pair is measured across a diagonal; a
            // pad facing another's flat side is measured by that side, and
            // rounding the far corners changes nothing.
            if dx <= 0.0 || dy <= 0.0 {
                continue;
            }
            let box_gap = (dx * dx + dy * dy).sqrt();
            // Exact where the two corners face along the same diagonal, an
            // upper bound otherwise: the arcs pull the nearest copper back by
            // `r * (sqrt(2) - 1)` each.
            let correction = (a.radius + b.radius) * (2f64.sqrt() - 1.0);
            worst = worst.max(correction);
            if box_gap < MIN_CLEARANCE_NM && box_gap + correction >= MIN_CLEARANCE_NM {
                flipped += 1;
            }
        }
    }
    (worst, flipped)
}

#[test]
fn the_correction_is_larger_than_the_clearance_it_is_measured_against() {
    // `charlieplex_3x3` has 26 rounded pads of its 34. Not a small number
    // beside the limit - which is why this file exists rather than a sentence
    // in the tracker saying the difference is negligible.
    let (worst, _) = corner_effect(
        "viewer/kicad-tools/boards/02-charlieplex-led/output/charlieplex_3x3.kicad_pcb",
    );
    assert!(
        worst > MIN_CLEARANCE_NM,
        "the corners on this board correct by {worst:.0}nm, and the clearance \
         they are measured against is {MIN_CLEARANCE_NM:.0}nm - if this has \
         become small, the reason this rule measures boxes is worth revisiting"
    );
}

#[test]
fn no_board_in_this_repository_has_a_verdict_that_turns_on_a_corner() {
    // The measurement that decides whether the rule is worth teaching about
    // corners. Zero on every board here: not one diagonal pair sits inside the
    // limit by its boxes and outside it by its copper. A board that fails this
    // is a board the checker refuses and a fab would make.
    for board in BOARDS {
        let (_, flipped) = corner_effect(board);
        assert_eq!(
            flipped, 0,
            "{board} has {flipped} pad pair(s) the checker refuses for a corner \
             that is not there - the clearance rule now has a reason to measure \
             the arc"
        );
    }
}

#[test]
fn a_pad_with_no_corner_stated_corrects_by_nothing() {
    // The guard above is only worth its cost while the radius is real. A
    // reader that lost the ratio would make every correction zero and both
    // cases above would pass while saying nothing.
    let pads = pads_of("viewer/kicad-tools/tests/fixtures/routing-diagnostic.kicad_pcb");
    let rounded = pads.iter().filter(|pad| pad.radius > 0.0).count();
    assert!(
        rounded > 0,
        "no pad on this board carries a corner radius, so the measurement above \
         is arithmetic on zeroes"
    );
    let smallest = pads
        .iter()
        .filter(|pad| pad.radius > 0.0)
        .map(|pad| pad.radius)
        .fold(f64::INFINITY, f64::min);
    // This board's rounded pads state 0.2 rather than KiCad's usual 0.25, and
    // the reader has carried the stated figure since 2026-08-31.
    assert!(
        smallest < Nm::from_mm(0.25).0 as f64,
        "the smallest radius here is {smallest:.0}nm"
    );
}

#[test]
fn nothing_in_this_repository_is_refused_for_a_corner_today() {
    // The other half of the same question, asked of the rule rather than of
    // the geometry: `ClearanceRule` reports nothing on any of these boards, so
    // no board here is being refused at all - with corners or without them.
    // The day one of them is refused, the case above says whether a corner is
    // the reason.
    let rules = DesignRules::jlcpcb_2layer();
    for board in BOARDS {
        let parsed = cypcb_kicad::parse_kicad_pcb(&repo_root().join(board))
            .unwrap_or_else(|error| panic!("{board}: {error}"));
        let mut world = parsed.world;
        let violations = ClearanceRule.check(&mut world, &rules);
        assert!(
            violations.is_empty(),
            "{board} is refused for clearance now ({} violation(s)) - check the \
             case above before changing anything: the checker measures pads as \
             boxes, and a rounded pad has more copper-to-copper gap than its box \
             says",
            violations.len()
        );
    }
}
