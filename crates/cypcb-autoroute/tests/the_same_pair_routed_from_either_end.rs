//! The same pair of pads, routed from either end.
//!
//! `cargo test --release -p cypcb-autoroute --test the_same_pair_routed_from_either_end -- --ignored --nocapture`
//!
//! The owner reported on 2026-09-10 that routing a connection from component A
//! to component B comes out better than routing the same pair from B to A. A
//! path finder whose answer depends on which end it started from is telling
//! you that its cost function, its tie-breaking or its obstacle marking is not
//! symmetric either - and nothing in this repository measured it.
//!
//! This measures it at the level the shipped router uses: one grid built the
//! way `PathFinderStrategy` builds it, and `find_path_with_zones` called twice
//! per connection - forward, then with the two ends exchanged - each on its
//! own copy of the grid, because a found path marks the cells it took.
//!
//! A symmetric search returns the reverse of the first path as the second. Two
//! readings are useful and neither is an opinion: the paths differ, and the
//! difference has a size in cells and in vias; or they do not, and what the
//! owner saw belongs to the viewer's own preview rather than to this router.

use std::path::Path;

use cypcb_autoroute::cost::RoutingCost;
use cypcb_autoroute::grid::RoutingGrid;
use cypcb_autoroute::orchestrator::{
    build_spanning_tree, extract_ratsnest, pad_to_grid_node, pad_to_zone,
};
use cypcb_autoroute::pathfinder::{find_path_with_zones, GridNode, PadZone};
use cypcb_autoroute::pathfinder_v2::PathFinderStrategy;
use cypcb_autoroute::AutorouteConfig;
use cypcb_drc::{preset_for_world, ruleset_for_world};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_rules::presets::RulesPreset;

/// Every benchmark fixture, in the order the ratchet table lists them.
const FIXTURES: &[&str] = &[
    "led_blink.kicad_pcb",
    "stm32_breakout.kicad_pcb",
    "multi_ic.kicad_pcb",
    "shift_driver.kicad_pcb",
    "qfp_fanout.kicad_pcb",
    "plane_board.kicad_pcb",
];

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// What one connection did when it was routed in each direction.
struct Pair {
    net: String,
    from: String,
    to: String,
    /// True when the two pads are not the same kind of endpoint: one of them
    /// reaches every layer and the other does not. The search is then asked a
    /// different question in each direction - `any_end_layer` is a property of
    /// the *end*, so it travels with the direction rather than with the pair.
    mixed_ends: bool,
    forward: Option<Vec<GridNode>>,
    backward: Option<Vec<GridNode>>,
}

impl Pair {
    /// A path's length in cells, and how many times it changed layer.
    fn shape(path: &Option<Vec<GridNode>>) -> (usize, usize) {
        match path {
            None => (0, 0),
            Some(nodes) => {
                let vias = nodes.windows(2).filter(|w| w[0].2 != w[1].2).count();
                (nodes.len(), vias)
            }
        }
    }

    /// True when the backward path is the forward path walked the other way.
    fn mirrored(&self) -> bool {
        match (&self.forward, &self.backward) {
            (Some(f), Some(b)) => {
                f.len() == b.len() && f.iter().rev().zip(b.iter()).all(|(a, c)| a == c)
            }
            (None, None) => true,
            _ => false,
        }
    }
}

/// Route every connection of every net in both directions on one board.
fn both_ways(fixture: &str) -> Vec<Pair> {
    let parsed =
        parse_kicad_pcb(&fixture_path(fixture)).unwrap_or_else(|e| panic!("{fixture}: {e:?}"));
    let mut world = parsed.world;
    let library = parsed.library;
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let rules = ruleset_for_world(preset, &world);
    let config = AutorouteConfig::default();

    let resolution = PathFinderStrategy::resolution_for(&mut world, &rules, &config);
    let grid = RoutingGrid::from_board_with_pads(
        &mut world,
        &library,
        &rules,
        resolution,
        config.pad_rect_extra_cells,
    )
    .expect("a grid");

    let ratsnest = extract_ratsnest(&mut world, &library);
    let mut pairs = Vec::new();

    for net in &ratsnest {
        let zones: Vec<PadZone> = net.pads.iter().map(|pad| pad_to_zone(&grid, pad)).collect();
        for conn in build_spanning_tree(&net.pads) {
            let from_pad = &net.pads[conn.from_idx];
            let to_pad = &net.pads[conn.to_idx];
            let a = pad_to_grid_node(&grid, from_pad);
            let b = pad_to_grid_node(&grid, to_pad);
            let a_any = from_pad.layer_mask.count_ones() > 1;
            let b_any = to_pad.layer_mask.count_ones() > 1;

            let cost = RoutingCost::new(
                &rules,
                net.net_id.id(),
                config.via_cost_multiplier,
                config.params.layer_preference,
                grid.layer_count(),
            );

            // Each direction gets its own copy: a found path marks its cells.
            let mut forward_grid = grid.clone();
            let forward = find_path_with_zones(&mut forward_grid, a, b, &cost, b_any, &zones);
            let mut backward_grid = grid.clone();
            let backward = find_path_with_zones(&mut backward_grid, b, a, &cost, a_any, &zones);

            pairs.push(Pair {
                net: net.net_name.clone(),
                from: from_pad.pin.clone(),
                to: to_pad.pin.clone(),
                mixed_ends: a_any != b_any,
                forward,
                backward,
            });
        }
    }

    pairs
}

#[test]
#[ignore = "slow: builds a grid and routes every connection of every fixture twice"]
fn how_much_does_the_answer_depend_on_the_direction() {
    println!(
        "\n{:<22} {:>6} {:>9} {:>9} {:>9} {:>10} {:>7} {:>9}",
        "board", "pairs", "mirrored", "cells+-", "vias+-", "one-sided", "mixed", "mixed+-"
    );

    for fixture in FIXTURES {
        let pairs = both_ways(fixture);
        let mirrored = pairs.iter().filter(|p| p.mirrored()).count();
        let mut cell_delta = 0i64;
        let mut via_delta = 0i64;
        let mut one_sided = 0usize;
        let mut mixed = 0usize;
        let mut mixed_differs = 0usize;
        let mut same_ends_differs = 0usize;

        for pair in &pairs {
            let (fc, fv) = Pair::shape(&pair.forward);
            let (bc, bv) = Pair::shape(&pair.backward);
            if pair.forward.is_some() != pair.backward.is_some() {
                one_sided += 1;
            }
            let differs = fc != bc || fv != bv;
            if pair.mixed_ends {
                mixed += 1;
                if differs {
                    mixed_differs += 1;
                }
            } else if differs {
                same_ends_differs += 1;
            }
            cell_delta += (fc as i64 - bc as i64).abs();
            via_delta += (fv as i64 - bv as i64).abs();
        }

        println!(
            "{:<22} {:>6} {:>9} {:>9} {:>9} {:>10} {:>7} {:>9}",
            fixture.trim_end_matches(".kicad_pcb"),
            pairs.len(),
            mirrored,
            cell_delta,
            via_delta,
            one_sided,
            mixed,
            mixed_differs
        );
        if same_ends_differs > 0 {
            println!(
                "  {} pairs with the same kind of endpoint at both ends still differ",
                same_ends_differs
            );
        }

        // The worst pair by cells, named, so a defect has somewhere to start.
        if let Some(worst) = pairs.iter().max_by_key(|p| {
            let (fc, _) = Pair::shape(&p.forward);
            let (bc, _) = Pair::shape(&p.backward);
            (fc as i64 - bc as i64).abs()
        }) {
            let (fc, fv) = Pair::shape(&worst.forward);
            let (bc, bv) = Pair::shape(&worst.backward);
            if fc != bc || fv != bv {
                println!(
                    "  worst: net {} pins {} -> {}: {} cells / {} vias forward, {} / {} back",
                    worst.net, worst.from, worst.to, fc, fv, bc, bv
                );
            }
        }
    }
}

/// Two pads of the same kind cost the same from either end.
///
/// The sweep above measured where the two directions disagree, and every
/// disagreement it found on all six fixtures sat on a pair whose ends are of
/// **different kinds**: one pad reaching every layer, the other reaching one.
/// `any_end_layer` is a property of the end rather than of the pair, so such a
/// connection is literally a different search each way - one direction may
/// finish on whichever layer is cheapest, the other must land on the layer it
/// was given, and it pays a via for that.
///
/// This holds the other half: where both ends are the same kind, the answer
/// costs the same in cells and in vias whichever end it started from. That is
/// the part a tie-break or an obstacle marked asymmetrically would break, and
/// it is the part that has to stay true while the mixed-end case is fixed.
///
/// The geometry is a separate question and is deliberately not asserted: equal
/// cost by a different route is what the sweep's `mirrored` column counts, and
/// on `qfp_fanout` that is 4 pairs of 90.
#[test]
fn the_same_kind_of_end_costs_the_same_either_way() {
    let mut checked = 0usize;
    let mut wrong: Vec<String> = Vec::new();

    for fixture in FIXTURES {
        for pair in both_ways(fixture) {
            if pair.mixed_ends {
                continue;
            }
            checked += 1;
            let (fc, fv) = Pair::shape(&pair.forward);
            let (bc, bv) = Pair::shape(&pair.backward);
            if fc != bc || fv != bv {
                wrong.push(format!(
                    "{}: net {} pins {} -> {}: {fc} cells / {fv} vias forward, {bc} / {bv} back",
                    fixture.trim_end_matches(".kicad_pcb"),
                    pair.net,
                    pair.from,
                    pair.to
                ));
            }
        }
    }

    // A reader that recognises nothing would pass an assertion about absence.
    assert!(
        checked >= 200,
        "the six fixtures should offer a few hundred same-kind connections, found {checked}"
    );
    assert!(
        wrong.is_empty(),
        "{} of {checked} same-kind connections cost different amounts by direction:\n  {}",
        wrong.len(),
        wrong.join("\n  ")
    );
}
