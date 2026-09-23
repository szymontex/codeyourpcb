//! A net the router joined and a later pass cut, reported.
//!
//! `cargo test -p cypcb-autoroute --test a_net_cut_after_routing_is_reported`
//!
//! Until 2026-09-23 `optimize_vias` checked the segment it laid in place of a
//! via pair against a list every caller passed empty, and it removed pairs that
//! another branch of the net climbed out of. On `led_blink` routed with
//! `stop_at_own_copper` that left C2.2 on a piece of GND joined to nothing
//! else, and every rule was quiet: `unrouted-pin` asks whether copper reaches
//! a pin, and copper did.
//!
//! This replays that elimination on the board the router lays today - every
//! pair of vias that goes down and comes back up with one segment between them
//! is replaced by a straight segment on the first layer, and nothing is
//! checked - and holds `net-split` to reporting what it does.

use std::path::{Path, PathBuf};

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{DrcRule, NetSplitRule, UnroutedPinRule};
use cypcb_drc::{preset_for_world, ruleset_for_world};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;
use cypcb_router::types::{RouteSegment, RoutingResult};

fn fixture_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// The via optimizer as it was, with its empty list: every complementary pair
/// joined by one segment on the other layer goes. Returns how many pairs.
fn replay_old_elimination(result: &mut RoutingResult) -> usize {
    let mut removed_vias: Vec<usize> = Vec::new();
    let mut removed_segments: Vec<usize> = Vec::new();
    let mut added: Vec<RouteSegment> = Vec::new();

    for i in 0..result.vias.len() {
        for j in (i + 1)..result.vias.len() {
            if removed_vias.contains(&i) || removed_vias.contains(&j) {
                continue;
            }
            let (a, b) = (&result.vias[i], &result.vias[j]);
            if a.net_id != b.net_id || a.start_layer != b.end_layer || a.end_layer != b.start_layer
            {
                continue;
            }
            let between = result.routes.iter().enumerate().position(|(k, s)| {
                !removed_segments.contains(&k)
                    && s.net_id == a.net_id
                    && s.layer == a.end_layer
                    && ((s.start == a.position && s.end == b.position)
                        || (s.start == b.position && s.end == a.position))
            });
            let Some(between) = between else {
                continue;
            };
            let segment = &result.routes[between];
            added.push(RouteSegment::new(
                a.net_id,
                a.start_layer,
                segment.width,
                a.position,
                b.position,
            ));
            removed_vias.extend([i, j]);
            removed_segments.push(between);
        }
    }

    let pairs = removed_vias.len() / 2;
    removed_vias.sort_unstable();
    for index in removed_vias.into_iter().rev() {
        result.vias.remove(index);
    }
    removed_segments.sort_unstable();
    for index in removed_segments.into_iter().rev() {
        result.routes.remove(index);
    }
    result.routes.extend(added);
    pairs
}

/// `led_blink` routed with the flag, optionally with the old elimination
/// replayed on it: what `net-split` and `unrouted-pin` say.
fn check(replay: bool) -> (usize, Vec<String>, usize) {
    let parsed = parse_kicad_pcb(&fixture_path("led_blink.kicad_pcb")).expect("the fixture parses");
    let mut world = parsed.world;
    let library = parsed.library;
    let preset = preset_for_world(
        cypcb_rules::presets::RulesPreset::JlcpcbStandard2Layer,
        &world,
    );
    let rules = ruleset_for_world(preset, &world);
    let config = AutorouteConfig {
        stop_at_own_copper: true,
        ..AutorouteConfig::default()
    };
    let mut result = route_board(&mut world, &library, &rules, &config);
    let pairs = if replay {
        replay_old_elimination(&mut result)
    } else {
        0
    };
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let drc = DesignRules::default();
    let splits = NetSplitRule
        .check(&mut world, &drc)
        .into_iter()
        .map(|violation| violation.message)
        .collect();
    let unrouted = UnroutedPinRule.check(&mut world, &drc).len();
    (pairs, splits, unrouted)
}

#[test]
fn the_board_the_router_lays_is_in_one_piece_per_net() {
    let (_, splits, unrouted) = check(false);
    assert!(splits.is_empty(), "{splits:?}");
    assert_eq!(unrouted, 0);
}

#[test]
fn the_old_via_optimizers_cut_is_reported_where_unrouted_pin_is_quiet() {
    let (pairs, splits, unrouted) = check(true);
    assert!(
        pairs > 0,
        "the router laid no via pair the old optimizer would have taken, so \
         this test replays nothing and proves nothing"
    );
    println!("pairs eliminated {pairs}; net-split: {splits:?}");
    assert_eq!(
        unrouted, 0,
        "every pin still has copper on it - that is why `unrouted-pin` missed it"
    );
    assert!(
        splits
            .iter()
            .any(|message| message.starts_with("net GND ") && message.contains("C2.2")),
        "the elimination cut C2.2 off GND and `net-split` did not say so: {splits:?}"
    );
}
