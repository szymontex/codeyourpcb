//! A benchmark fixture that cannot be fabricated poisons every number drawn
//! from it.
//!
//! `cargo test -p cypcb-autoroute --test every_fixture_is_a_board_somebody_could_make`
//!
//! `qfp_fanout` shipped with two 21-pin headers on a 46mm board - 50.8mm of
//! pins, so three pads per header sat past the outline. It was found by reading
//! the coordinates of its violations, which clustered at y = 45.5 to 46.1 on a
//! board 46mm tall, and by then it had been scored against for two commits with
//! its ratchet in CI.
//!
//! Nothing had ever asked the obvious question of a fixture: is this a board?
//! These tests ask it of all five, so the next one added answers before it is
//! measured on.

use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_world::components::{FootprintRef, Position, Rotation};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use std::path::{Path, PathBuf};

fn fixture_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// Every pad's copper, in board coordinates, as (refdes-ish label, box).
fn pad_boxes(world: &mut BoardWorld, library: &FootprintLibrary) -> Vec<(String, [i64; 4])> {
    let placed: Vec<(cypcb_core::Point, String, f64)> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(&Position, &FootprintRef, &Rotation)>();
        query
            .iter(ecs)
            .map(|(position, footprint, rotation)| {
                (position.0, footprint.0.clone(), rotation.to_degrees())
            })
            .collect()
    };

    let mut boxes = Vec::new();
    for (position, name, rotation_deg) in placed {
        let Some(footprint) = library.get(&name) else {
            continue;
        };
        let (sin, cos) = rotation_deg.to_radians().sin_cos();
        for pad in &footprint.pads {
            let x = pad.position.x.raw() as f64;
            let y = pad.position.y.raw() as f64;
            let cx = position.x.raw() + (x * cos - y * sin).round() as i64;
            let cy = position.y.raw() + (x * sin + y * cos).round() as i64;
            // The pad's own box, turned with the part. Taking
            // `max(width, height)` for both axes instead - the bound the
            // routing grid uses - reported eleven pads of `qfp_fanout` as off
            // the board when a check against their real extents found none:
            // an LQFP pad is 1.5 by 0.3mm, and squaring it adds 0.6mm of
            // nothing. A measurement that is conservative for a keepout is
            // simply wrong for a question about where the copper is.
            let (half_w, half_h) = if (sin.abs() - 1.0).abs() < 1e-9 {
                (pad.size.1.raw() / 2, pad.size.0.raw() / 2)
            } else if sin.abs() < 1e-9 {
                (pad.size.0.raw() / 2, pad.size.1.raw() / 2)
            } else {
                // An angle that is not a right angle: the circumscribing box.
                let w = pad.size.0.raw() as f64;
                let h = pad.size.1.raw() as f64;
                (
                    ((w * cos).abs() + (h * sin).abs()) as i64 / 2,
                    ((w * sin).abs() + (h * cos).abs()) as i64 / 2,
                )
            };
            boxes.push((
                format!("{name} pad {}", pad.number),
                [cx - half_w, cy - half_h, cx + half_w, cy + half_h],
            ));
        }
    }
    boxes
}

/// The three fixtures this does not hold for yet, with what they carry.
///
/// `stm32_breakout` runs two ten-pin headers past its 65mm height; `multi_ic`
/// places an inductor and an Ethernet transformer at negative x, 50mm to the
/// left of the board; `shift_driver` has its bypass capacitors 3mm above the
/// top edge, put there to clear a DIP courtyard. Every routing number this
/// project has published rests on the first two.
///
/// Fixing them moves every ratchet and every table in `docs/routing.md`, so it
/// is a decision rather than a fire, and until it is taken this test is a
/// diagnostic rather than a gate.
const KNOWN_OFF_BOARD: &[(&str, usize)] = &[
    ("stm32_breakout.kicad_pcb", 8),
    ("multi_ic.kicad_pcb", 10),
    ("shift_driver.kicad_pcb", 6),
];

#[test]
fn no_fixture_has_copper_outside_its_own_board_outline() {
    let mut broken = Vec::new();

    for benchmark in BENCHMARKS {
        let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
            .unwrap_or_else(|e| panic!("failed to parse {}: {e:?}", benchmark.filename));
        let mut world = parsed.world;

        let Some((size, _)) = world.board_info() else {
            broken.push(format!("{}: states no board size", benchmark.filename));
            continue;
        };
        let (width, height) = (size.width.raw(), size.height.raw());

        for (label, [min_x, min_y, max_x, max_y]) in pad_boxes(&mut world, &parsed.library) {
            if min_x < 0 || min_y < 0 || max_x > width || max_y > height {
                broken.push(format!(
                    "{}: {label} spans x {min_x}..{max_x} y {min_y}..{max_y}, \
                     board is {width} x {height}",
                    benchmark.filename
                ));
            }
        }
    }

    // Counted per fixture and held against what is known, so a new fixture
    // with copper off the board fails here and the three that already do are
    // not silently forgotten.
    for benchmark in BENCHMARKS {
        let found = broken
            .iter()
            .filter(|line| line.starts_with(benchmark.filename))
            .count();
        let known = KNOWN_OFF_BOARD
            .iter()
            .find(|(name, _)| *name == benchmark.filename)
            .map(|(_, count)| *count)
            .unwrap_or(0);

        assert_eq!(
            found,
            known,
            "{}: {found} pads outside the board outline, {known} known. \
             A fixture with copper off the board is not a board, and every \
             number measured on it is measured on nothing.\n{}",
            benchmark.filename,
            broken
                .iter()
                .filter(|line| line.starts_with(benchmark.filename))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

#[test]
fn every_fixture_states_a_size_and_a_layer_count() {
    // The other half of "is this a board". A fixture with no size gets the
    // 100x100mm default, silently, and the router is then measured on a board
    // the file never described.
    for benchmark in BENCHMARKS {
        let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
            .unwrap_or_else(|e| panic!("failed to parse {}: {e:?}", benchmark.filename));
        let world = parsed.world;

        let (size, stack) = world
            .board_info()
            .unwrap_or_else(|| panic!("{} states no board", benchmark.filename));

        assert!(
            size.width.raw() > 0 && size.height.raw() > 0,
            "{} has a board of no area",
            benchmark.filename
        );
        assert!(
            stack.count >= 2,
            "{} claims {} copper layers",
            benchmark.filename,
            stack.count
        );
    }
}
