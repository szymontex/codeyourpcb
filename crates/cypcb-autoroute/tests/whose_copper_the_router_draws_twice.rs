//! Is the duplicated copper a shared trunk, or a path doubling back?
//!
//! `cargo test -p cypcb-autoroute --test whose_copper_the_router_draws_twice -- --nocapture`
//!
//! 142 of the 195 acute reports on the benchmark boards are copper laid back
//! along copper, and 129 of those are inside the router's own output. Two
//! mechanisms produce that and they need opposite fixes.
//!
//! A **shared trunk**: a net with several pins is decomposed into two-pin
//! connections and each is routed from pad to pad on its own, so two
//! connections that want the same corridor both write it. The router's own
//! copper is marked with the net's id and is cheap rather than forbidden -
//! nothing makes it a destination, and nothing merges two runs that came from
//! different paths. `merge_collinear` joins collinear neighbours within one
//! chain, and two overlapping runs from two paths are not a chain.
//!
//! A **path doubling back**: one search returns a route that revisits a cell.
//!
//! The two are told apart on data that already exists. `pathfinder_loop` is
//! callable from outside and its `routed_paths` is public: a map from net to
//! the list of grid paths, one per connection. Cells shared between two paths
//! of one net are the trunk; cells repeated inside one path are the doubling
//! back. Neither needs a new field and neither changes what the router does.
//!
//! The counts here are in grid cells and the 129 is in nanometre segments - a
//! trunk N cells long collapses into one segment or a few. The two are not
//! subtractable and the test does not subtract them; what carries across is
//! the shape per board.

use std::collections::BTreeMap;
use std::path::Path;

use cypcb_autoroute::grid::RoutingGrid;
use cypcb_autoroute::orchestrator::{extract_ratsnest, order_nets};
use cypcb_autoroute::pathfinder::GridNode;
use cypcb_autoroute::pathfinder_v2::pathfinder_loop;
use cypcb_autoroute::AutorouteConfig;
use cypcb_drc::{preset_for_world, ruleset_for_world};
use cypcb_kicad::parse_kicad_pcb;

const FIXTURES: &[&str] = &[
    "led_blink.kicad_pcb",
    "stm32_breakout.kicad_pcb",
    "multi_ic.kicad_pcb",
    "shift_driver.kicad_pcb",
    "qfp_fanout.kicad_pcb",
    "plane_board.kicad_pcb",
];

/// What one board's grid paths hold.
struct Counts {
    /// Cells that appear in at least two different paths of one net.
    trunk: usize,
    /// The longest unbroken run of such cells within a single path.
    longest_trunk: usize,
    /// Cells that appear twice or more inside one path.
    doubled: usize,
    /// The closest pair of indices at which one path revisits a cell.
    tightest_double: usize,
}

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// Run the search on one board and count both mechanisms in its grid paths.
fn counted(fixture: &str) -> Counts {
    let parsed = parse_kicad_pcb(&fixture_path(fixture))
        .unwrap_or_else(|e| panic!("failed to parse {fixture}: {e:?}"));
    let mut world = parsed.world;
    let library = parsed.library;
    let preset = preset_for_world(
        cypcb_rules::presets::RulesPreset::JlcpcbStandard2Layer,
        &world,
    );
    let rules = ruleset_for_world(preset, &world);
    let config = AutorouteConfig::default();

    let resolution = match world.board_info() {
        Some((size, _)) => {
            config.resolve_adaptive_grid_resolution(&rules, size.width.raw(), size.height.raw())
        }
        None => config.resolve_grid_resolution(&rules),
    };
    let mut grid = RoutingGrid::from_board(&mut world, &library, &rules, resolution)
        .unwrap_or_else(|| panic!("{fixture} has no grid"));

    let ratsnest = extract_ratsnest(&mut world, &library);
    let order = order_nets(&ratsnest);
    let result = pathfinder_loop(&mut grid, &ratsnest, &order, &rules, &config, None);

    let mut counts = Counts {
        trunk: 0,
        longest_trunk: 0,
        doubled: 0,
        tightest_double: usize::MAX,
    };

    for paths in result.routed_paths.values() {
        count_one_nets_paths(paths, &mut counts);
    }

    counts
}

/// Add one net's paths to the running counts.
///
/// Held by two unit tests below rather than by the boards: on every fixture
/// the doubling count is zero, so an assertion about it out there proves
/// nothing about whether this can see one.
fn count_one_nets_paths(paths: &[Vec<GridNode>], counts: &mut Counts) {
    // How many of this net's paths each cell belongs to. A cell a single path
    // enters twice belongs to one path, so the visits are deduplicated before
    // they are added up - otherwise a path doubling back would read as two
    // paths sharing a trunk, which is the other mechanism entirely.
    let mut owners: BTreeMap<GridNode, usize> = BTreeMap::new();
    for path in paths {
        let mut here: Vec<GridNode> = path.clone();
        here.sort();
        here.dedup();
        for cell in here {
            *owners.entry(cell).or_default() += 1;
        }
    }
    counts.trunk += owners.values().filter(|count| **count > 1).count();

    for path in paths {
        // A trunk is a long unbroken stretch shared with another path; a
        // chance touch is one cell or two, and counting them the same would
        // call every crossing a trunk.
        let mut run = 0;
        for cell in path {
            if owners.get(cell).copied().unwrap_or(0) > 1 {
                run += 1;
                counts.longest_trunk = counts.longest_trunk.max(run);
            } else {
                run = 0;
            }
        }

        // A cell this one path enters twice, and how few steps apart.
        let mut first_seen: BTreeMap<GridNode, usize> = BTreeMap::new();
        for (index, cell) in path.iter().enumerate() {
            if let Some(earlier) = first_seen.insert(*cell, index) {
                counts.doubled += 1;
                counts.tightest_double = counts.tightest_double.min(index - earlier);
            }
        }
    }
}

#[test]
fn the_duplicated_copper_is_a_shared_trunk() {
    let mut trunk = 0;
    let mut doubled = 0;
    let mut per_board = Vec::new();

    for fixture in FIXTURES {
        let counts = counted(fixture);
        println!(
            "{fixture:<26} trunk cells {:>6}, longest run {:>4}   \
             doubled cells {:>5}, tightest {}",
            counts.trunk,
            counts.longest_trunk,
            counts.doubled,
            if counts.tightest_double == usize::MAX {
                "none".to_string()
            } else {
                counts.tightest_double.to_string()
            }
        );
        trunk += counts.trunk;
        doubled += counts.doubled;
        per_board.push((*fixture, counts.trunk));
    }
    println!("all six boards: {trunk} trunk cells, {doubled} doubled cells");

    // The control. A run that found nothing would satisfy any ratio between
    // the two, and a search that returned no paths would find nothing.
    assert!(
        trunk + doubled > 0,
        "the search returned no cell shared by anything"
    );

    // The verdict, on the rule agreed before the numbers were seen: a trunk
    // at five times the doubling is the first mechanism, a doubling at or
    // above the trunk is the second, and anything between is both at once and
    // gets no verdict.
    assert!(
        trunk >= 5 * doubled,
        "{trunk} trunk cells against {doubled} doubled cells is not a verdict \
         for the shared trunk, so the second mechanism is present too"
    );
}

#[test]
fn the_boards_that_duplicate_nothing_share_no_run_longer_than_a_cell() {
    // The cross-check against the measurement taken in nanometre segments,
    // which cannot be compared by magnitude: a trunk N cells long collapses
    // into one segment or a few. What carries across is which boards have it.
    //
    // Measured 2026-09-11, per board - duplicated junctions in the router's
    // own output, then the longest run of cells it shares between two paths of
    // one net: led_blink 0/1, stm32_breakout 10/4, multi_ic 30/6,
    // shift_driver 2/13, qfp_fanout 87/53, plane_board 0/1.
    //
    // The two boards that duplicate nothing are exactly the two whose longest
    // shared run is a single cell, and a single shared cell is two paths
    // crossing rather than two paths running together. `qfp_fanout` holds 87
    // of the 129 duplications and both the most shared cells and the longest
    // shared run.
    let measured: Vec<(&str, usize, usize)> = FIXTURES
        .iter()
        .map(|fixture| {
            let counts = counted(fixture);
            (*fixture, counts.trunk, counts.longest_trunk)
        })
        .collect();

    let quiet = ["led_blink.kicad_pcb", "plane_board.kicad_pcb"];
    for (fixture, _, longest) in &measured {
        if quiet.contains(fixture) {
            assert_eq!(
                *longest, 1,
                "{fixture} duplicates no copper, so its paths should only ever \
                 cross, but they run together for {longest} cells"
            );
        } else {
            assert!(
                *longest > 1,
                "{fixture} duplicates copper, so two of its paths have to run \
                 together for more than the one cell of a crossing"
            );
        }
    }

    let most = measured
        .iter()
        .max_by_key(|(_, trunk, _)| *trunk)
        .expect("six boards");
    assert_eq!(
        most.0, "qfp_fanout.kicad_pcb",
        "the board with the most duplicated copper is qfp_fanout and the board \
         with the most shared cells is {}",
        most.0
    );
}

#[test]
fn a_path_that_comes_back_to_a_cell_is_counted_as_doubling_back() {
    // The positive control for the second mechanism. Every fixture measures
    // zero of these, so without a doubling built by hand the verdict above
    // would hold just as well against a counter that cannot count.
    let path = vec![(0, 0, 0), (1, 0, 0), (2, 0, 0), (1, 0, 0)];
    let mut counts = Counts {
        trunk: 0,
        longest_trunk: 0,
        doubled: 0,
        tightest_double: usize::MAX,
    };
    count_one_nets_paths(std::slice::from_ref(&path), &mut counts);

    assert_eq!(counts.doubled, 1, "the path returns to (1, 0) at index 3");
    assert_eq!(
        counts.tightest_double, 2,
        "two steps after it first left it"
    );
    assert_eq!(
        counts.trunk, 0,
        "one path revisiting a cell is not two paths sharing one"
    );
}

#[test]
fn a_cell_is_shared_by_paths_not_by_visits() {
    // The control for the first mechanism, and for the deduplication that
    // separates the two. Counting visits instead of paths would report the
    // doubling above as a trunk and put the verdict the other way round.
    let first = vec![(0, 0, 0), (1, 0, 0), (2, 0, 0)];
    let second = vec![(1, 0, 0), (1, 1, 0)];
    let mut counts = Counts {
        trunk: 0,
        longest_trunk: 0,
        doubled: 0,
        tightest_double: usize::MAX,
    };
    count_one_nets_paths(&[first, second], &mut counts);

    assert_eq!(counts.trunk, 1, "only (1, 0) is on both paths");
    assert_eq!(counts.longest_trunk, 1, "and they share it for one cell");
    assert_eq!(counts.doubled, 0, "neither path returns anywhere");
}
