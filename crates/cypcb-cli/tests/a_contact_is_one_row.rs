//! A contact between two nets is one clearance row however the copper is cut
//! into trace entities.
//!
//! `cargo test -p cypcb-cli --test a_contact_is_one_row`
//!
//! The router holds one trace entity per net and layer, the reader one per
//! `path` line, and nothing about the copper says which is right. The
//! clearance rule used to count per pair of entities, so the same board
//! scored 44 shorts in memory and 46 read back from its own file, and 61 when
//! every segment was an entity of its own. Each board here is routed once and
//! then cut several ways - merged per net and layer, one entity per segment,
//! each trace dealt into two interleaved pieces, every segment turned round -
//! and every cut has to report the same clearance rows: the same places, the
//! same names, the same distances.
//!
//! One more cut changes the segments rather than the entities: every segment
//! split in two at its middle. The copper is the same and so is the number of
//! places, but where a place is reported can move to the new vertex, so that
//! cut is held to the same rows with the coordinate left out.

use std::path::{Path, PathBuf};

use cypcb_drc::{run_drc, PresetRules, ViolationKind};
use cypcb_rules::presets::RulesPreset;
use cypcb_world::components::trace::{Trace, TraceSegment};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld, Entity};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(name)
}

fn load(path: &Path) -> (BoardWorld, FootprintLibrary) {
    if path.extension().and_then(|e| e.to_str()) == Some("kicad_pcb") {
        let parsed = cypcb_kicad::parse_kicad_pcb(path).expect("the board reads");
        let mut world = parsed.world;
        let library = parsed.library;
        world.set_footprints(library.clone());
        world.rebuild_spatial_index_from_library(&library);
        (world, library)
    } else {
        let source = std::fs::read_to_string(path).expect("the board is there");
        let parsed = cypcb_parser::parse(&source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let synced = sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);
        assert!(synced.errors.is_empty(), "{:?}", synced.errors);
        (world, library)
    }
}

/// The ways the same copper is cut into entities.
#[derive(Clone, Copy, Debug)]
enum Cut {
    /// One entity per net, layer and width - the router's grouping.
    NetAndLayer,
    /// Every segment an entity of its own.
    EverySegment,
    /// Each trace dealt into two entities, segment by segment, so neither
    /// piece is a connected run.
    Interleaved,
    /// Each trace as it was, every segment written from its other end and the
    /// segments in reverse order.
    TurnedRound,
    /// Each trace as it was, every segment cut in two at its middle.
    SplitInHalf,
}

fn cut(world: &mut BoardWorld, library: &FootprintLibrary, how: Cut) {
    let traces: Vec<(Entity, Trace)> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(Entity, &Trace)>();
        query.iter(ecs).map(|(e, t)| (e, t.clone())).collect()
    };
    for (entity, _) in &traces {
        world.ecs_mut().despawn(*entity);
    }
    let mut pieces: Vec<Trace> = Vec::new();
    match how {
        Cut::NetAndLayer => {
            let mut groups: std::collections::BTreeMap<_, Trace> = Default::default();
            for (_, trace) in &traces {
                let key = (
                    trace.net_id.id(),
                    format!("{:?}", trace.layer),
                    trace.width.0,
                    trace.locked,
                    format!("{:?}", trace.source),
                );
                groups
                    .entry(key)
                    .or_insert_with(|| Trace {
                        segments: Vec::new(),
                        ..trace.clone()
                    })
                    .segments
                    .extend(trace.segments.iter().copied());
            }
            pieces.extend(groups.into_values());
        }
        Cut::EverySegment => {
            for (_, trace) in &traces {
                for segment in &trace.segments {
                    pieces.push(Trace {
                        segments: vec![*segment],
                        ..trace.clone()
                    });
                }
            }
        }
        Cut::Interleaved => {
            for (_, trace) in &traces {
                for parity in 0..2 {
                    let segments: Vec<TraceSegment> = trace
                        .segments
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| i % 2 == parity)
                        .map(|(_, s)| *s)
                        .collect();
                    if !segments.is_empty() {
                        pieces.push(Trace {
                            segments,
                            ..trace.clone()
                        });
                    }
                }
            }
        }
        Cut::TurnedRound => {
            for (_, trace) in &traces {
                let segments = trace
                    .segments
                    .iter()
                    .rev()
                    .map(|s| TraceSegment {
                        start: s.end,
                        end: s.start,
                        ..*s
                    })
                    .collect();
                pieces.push(Trace {
                    segments,
                    ..trace.clone()
                });
            }
        }
        Cut::SplitInHalf => {
            for (_, trace) in &traces {
                let segments = trace
                    .segments
                    .iter()
                    .flat_map(|s| {
                        let middle = cypcb_core::Point::new(
                            cypcb_core::Nm((s.start.x.0 + s.end.x.0) / 2),
                            cypcb_core::Nm((s.start.y.0 + s.end.y.0) / 2),
                        );
                        [
                            TraceSegment { end: middle, ..*s },
                            TraceSegment {
                                start: middle,
                                ..*s
                            },
                        ]
                    })
                    .collect();
                pieces.push(Trace {
                    segments,
                    ..trace.clone()
                });
            }
        }
    }
    for piece in pieces {
        let net = piece.net_id;
        world.spawn_entity((piece, net));
    }
    world.rebuild_spatial_index_from_library(library);
}

/// Every clearance row as place, message, distance and rule, sorted.
fn clearance_rows(world: &mut BoardWorld) -> Vec<String> {
    clearance_rows_placed(world, true)
}

fn clearance_rows_placed(world: &mut BoardWorld, placed: bool) -> Vec<String> {
    let preset = match world.fab() {
        None => RulesPreset::JlcpcbStandard2Layer,
        Some(name) if name.eq_ignore_ascii_case("jlcpcb") => RulesPreset::JlcpcbStandard2Layer,
        Some(name) => panic!("fab {name}: map it here"),
    };
    let preset = cypcb_drc::preset_for_world(preset, world);
    let mut rows: Vec<String> = run_drc(world, &preset.rules())
        .violations
        .iter()
        .filter(|v| v.kind == ViolationKind::Clearance)
        .map(|v| {
            let place = if placed {
                format!("({}, {}) ", v.location.x.0, v.location.y.0)
            } else {
                String::new()
            };
            format!("{place}{} {:?} {:?}", v.message, v.actual, v.required)
        })
        .collect();
    rows.sort();
    rows
}

fn every_cut_agrees(name: &str) {
    let (mut world, library) = load(&fixture(name));
    let rules = cypcb_drc::ruleset_for_world(
        cypcb_drc::preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world),
        &world,
    );
    let result = cypcb_autoroute::route_board(
        &mut world,
        &library,
        &rules,
        &cypcb_autoroute::AutorouteConfig::default(),
    );
    cypcb_router::apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let as_routed = clearance_rows(&mut world);
    let as_routed_unplaced = clearance_rows_placed(&mut world, false);
    for how in [
        Cut::EverySegment,
        Cut::Interleaved,
        Cut::TurnedRound,
        Cut::NetAndLayer,
    ] {
        cut(&mut world, &library, how);
        let rows = clearance_rows(&mut world);
        assert_rows(name, how, &as_routed, &rows);
    }
    // Last, and on the router's grouping again: a vertex in the middle of a
    // straight is still the same copper.
    cut(&mut world, &library, Cut::SplitInHalf);
    let rows = clearance_rows_placed(&mut world, false);
    assert_rows(name, Cut::SplitInHalf, &as_routed_unplaced, &rows);
}

fn assert_rows(name: &str, how: Cut, as_routed: &[String], rows: &[String]) {
    let lost: Vec<&String> = as_routed.iter().filter(|r| !rows.contains(r)).collect();
    let gained: Vec<&String> = rows.iter().filter(|r| !as_routed.contains(r)).collect();
    assert!(
        rows == as_routed,
        "{name} cut {how:?}: {} clearance rows as routed, {} after the cut; \
         lost {:?}; gained {:?}",
        as_routed.len(),
        rows.len(),
        lost.iter().take(5).collect::<Vec<_>>(),
        gained.iter().take(5).collect::<Vec<_>>()
    );
}

#[test]
fn esp32_starter() {
    every_cut_agrees("esp32_starter.cypcb");
}

#[test]
fn led_blink() {
    every_cut_agrees("led_blink.kicad_pcb");
}

#[test]
fn multi_ic() {
    every_cut_agrees("multi_ic.kicad_pcb");
}

#[test]
fn plane_board() {
    every_cut_agrees("plane_board.kicad_pcb");
}

#[test]
fn qfp_fanout() {
    every_cut_agrees("qfp_fanout.kicad_pcb");
}

#[test]
fn shift_driver() {
    every_cut_agrees("shift_driver.kicad_pcb");
}

#[test]
fn stm32_breakout() {
    every_cut_agrees("stm32_breakout.kicad_pcb");
}

/// Every benchmark board is cut above: one added without a line here would be
/// a board this promise quietly does not cover.
#[test]
fn every_benchmark_board_is_cut() {
    let this_file = include_str!("a_contact_is_one_row.rs");
    let mut boards: Vec<String> = std::fs::read_dir(fixture(""))
        .expect("the benchmark directory is there")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".kicad_pcb") || name.ends_with(".cypcb"))
        .collect();
    boards.sort();
    assert!(boards.len() >= 7, "found only {boards:?}");
    for board in boards {
        assert!(
            this_file.contains(&format!("every_cut_agrees(\"{board}\")")),
            "{board} is a benchmark board this file does not cut"
        );
    }
}
