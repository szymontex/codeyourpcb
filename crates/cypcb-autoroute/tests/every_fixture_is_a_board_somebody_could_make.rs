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

/// Fixtures with copper outside their own outline.
///
/// Empty, and it took four repairs to get there. `qfp_fanout` ran two 21-pin
/// headers along a 46mm board - 50.8mm of pins - and was rebuilt with four
/// headers of twelve. `multi_ic` carried `(at 105, 80)` with a comma, which
/// the importer read as zero and put two parts 50mm off the board.
/// `stm32_breakout` had two ten-pin headers whose last four pins ran 7.9mm
/// past its top edge, and they moved from y = 90 to y = 80. `shift_driver`
/// had three bypass capacitors 3.4mm above the board, put there to clear a
/// DIP courtyard, and they moved to y = 1.5 inside it.
///
/// A new entry here is a new defect rather than an inheritance.
const KNOWN_OFF_BOARD: &[(&str, usize)] = &[];

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

/// Parts that overlap each other, per fixture.
///
/// A courtyard is the room a part needs to be placed and soldered, so two that
/// overlap are two parts that cannot both go on the board.
///
/// The first count taken here was 26 on `multi_ic`, 2 on `stm32_breakout` and
/// 1 on `plane_board`. **More than half of those were manufactured by the
/// importer**, which derived a courtyard as the pad bounds plus a flat 0.5mm -
/// twice `IPC_COURTYARD_EXCESS`, the excess this project uses everywhere else -
/// so an imported board lost half a millimetre of apparent clearance between
/// every neighbouring pair. Derived at the right excess: 12, 0 and 0.
///
/// The twelve left on `multi_ic` were real placement in a hand-written fixture
/// and are repaired: nine decoupling capacitors ringing U1 at 0.14mm where
/// 0.25mm is required, an HC49 crystal 13.4mm wide across its neighbours, and
/// two parts against the Ethernet transformer. Its routed count moved 304 ->
/// 291 violations with 187 shorts, and eight solder-mask bridges went with
/// them.
///
/// Every fixture is now a board whose parts fit beside each other, so this
/// list is empty and a new entry in it is a new defect rather than an
/// inheritance.
const KNOWN_OVERLAPS: &[(&str, usize)] = &[];

#[test]
fn no_fixture_asks_for_two_parts_in_the_same_place() {
    use cypcb_drc::presets::DesignRules;
    use cypcb_drc::rules::{CourtyardClearanceRule, DrcRule};

    for benchmark in BENCHMARKS {
        let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
            .unwrap_or_else(|e| panic!("failed to parse {}: {e:?}", benchmark.filename));
        let mut world = parsed.world;
        world.rebuild_spatial_index_from_library(&parsed.library);

        let violations = CourtyardClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        let known = KNOWN_OVERLAPS
            .iter()
            .find(|(name, _)| *name == benchmark.filename)
            .map(|(_, count)| *count)
            .unwrap_or(0);

        assert_eq!(
            violations.len(),
            known,
            "{}: {} pairs of parts overlap, {known} known. Two parts in the \
             same place is not a board somebody could make, and every number \
             measured on it is measured on a board nobody can build.\n{}",
            benchmark.filename,
            violations.len(),
            violations
                .iter()
                .map(|v| v.message.clone())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
