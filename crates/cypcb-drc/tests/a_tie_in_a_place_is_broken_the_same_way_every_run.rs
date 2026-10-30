//! Two contacts equally bad in one place are told apart the same way every run.
//!
//! `cargo test -p cypcb-drc --test a_tie_in_a_place_is_broken_the_same_way_every_run`
//!
//! A place is reported once, at its worst contact. When two contacts are
//! equally deep at the same point - a net whose copper is cut into two
//! entities over the same line, which a routed board and a hand-edited one
//! both produce - nothing said which of them the row was built from, so the
//! entities the row names were whichever the spatial index handed over first,
//! and the places were listed in the order a hash map returned them. Sorted by
//! position that order cannot move on any board with places a point apart;
//! the entities could.
//!
//! The board here is a straight line of net 1 and a zigzag of net 2 that comes
//! to 0.05mm of it twenty times: twenty places between one pair of sides, the
//! smallest board with a hash map of more than one entry in it. Each test
//! reads the whole report, not a count.

use cypcb_core::{Nm, Point};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{ClearanceRule, DrcRule};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{Layer, NetId};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// Spawn a trace and say which entity it is.
fn trace(world: &mut BoardWorld, net: u32, points: &[(f64, f64)]) -> u32 {
    let segments = points
        .windows(2)
        .map(|w| {
            TraceSegment::new(
                Point::from_mm(w[0].0, w[0].1),
                Point::from_mm(w[1].0, w[1].1),
            )
        })
        .collect();
    let net = NetId::new(net);
    world
        .spawn_entity((
            Trace {
                segments,
                width: Nm::from_mm(0.1),
                layer: Layer::TopCopper,
                net_id: Some(net),
                locked: false,
                source: TraceSource::Autorouted,
            },
            net,
        ))
        .index()
}

/// What the board has, in the order it was spawned: the line, a second copy of
/// the line if asked, and the zigzag.
struct Board {
    world: BoardWorld,
    line: u32,
    copy: Option<u32>,
    zigzag: u32,
}

fn board(copy: bool, zigzag_first: bool) -> Board {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(60.0), Nm::from_mm(30.0)), 2);
    let mut zig = Vec::new();
    for k in 0..20 {
        let x = 2.0 + f64::from(k) * 1.8;
        zig.push((x, 12.0));
        zig.push((x + 0.9, 10.15));
    }
    zig.push((2.0 + 20.0 * 1.8, 12.0));
    let line = [(2.0, 10.0), (40.0, 10.0)];

    let zigzag_early = zigzag_first.then(|| trace(&mut world, 2, &zig));
    let line_id = trace(&mut world, 1, &line);
    let copy_id = copy.then(|| trace(&mut world, 1, &line));
    let zigzag = zigzag_early.unwrap_or_else(|| trace(&mut world, 2, &zig));
    world.rebuild_spatial_index_from_library(&FootprintLibrary::new());
    Board {
        world,
        line: line_id,
        copy: copy_id,
        zigzag,
    }
}

/// The whole report: where, how deep, and the entities each row names.
fn report(board: &mut Board) -> Vec<(i64, i64, i64, u32, u32)> {
    ClearanceRule
        .check(&mut board.world, &DesignRules::jlcpcb_2layer())
        .iter()
        .map(|v| {
            (
                v.location.x.0,
                v.location.y.0,
                v.actual.unwrap().0,
                v.entity.index(),
                v.other_entity.unwrap().index(),
            )
        })
        .collect()
}

#[test]
fn the_board_has_twenty_places_between_one_pair_of_sides() {
    let rows = report(&mut board(false, false));
    assert_eq!(rows.len(), 20);
    assert!(rows.windows(2).all(|w| w[0].0 < w[1].0), "by position");
    assert!(rows.iter().all(|r| r.2 == 50_000), "each 0.05mm deep");
}

#[test]
fn the_same_board_gives_the_same_report_every_time() {
    let first = report(&mut board(true, false));
    for _ in 0..50 {
        assert_eq!(report(&mut board(true, false)), first);
    }
}

#[test]
fn two_copies_of_the_line_are_named_by_the_lower_one_whichever_was_spawned_first() {
    for zigzag_first in [false, true] {
        let mut b = board(true, zigzag_first);
        let rows = report(&mut b);
        assert_eq!(rows.len(), 20, "a copy of the line does not add places");
        let lower = b.line.min(b.copy.unwrap());
        for row in &rows {
            assert_eq!((row.3, row.4), (lower, b.zigzag), "{row:?}");
        }
    }
}

#[test]
fn control_without_a_copy_the_line_names_every_row() {
    for zigzag_first in [false, true] {
        let mut b = board(false, zigzag_first);
        for row in report(&mut b) {
            assert_eq!((row.3, row.4), (b.line, b.zigzag), "{row:?}");
        }
    }
}
