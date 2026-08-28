//! The shape a pad blocks is the shape that was asked for.
//!
//! `cargo test -p cypcb-autoroute --test the_pad_shape_is_the_one_asked_for`
//!
//! The grid marked a disc of the pad's longer half-side until 2026-08-28,
//! which over-blocks an oblong pad by the difference between its sides. It
//! marks the pad's own rectangle now, with two cells of reach beyond the
//! clearance - a figure `pad_obstacle_shape_sweep` measured rather than
//! chose. These cases hold both halves: the default is the swept figure, and
//! the rectangle really is the smaller shape.

use cypcb_autoroute::grid::RoutingGrid;
use cypcb_autoroute::AutorouteConfig;
use cypcb_parser::parse;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// One 2.0mm by 0.6mm pad on a small board: the case the disc gets wrong.
const BOARD: &str = "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\n\
     footprint WIDE_PAD {\n    courtyard 3mm x 2mm\n    \
     pad 1 rect at 0mm, 0mm size 2mm x 0.6mm\n}\n\n\
     net SIG {\n}\n\ncomponent U1 connector \"WIDE_PAD\" {\n    at 10mm, 10mm\n    \
     pin.1 = SIG\n}\n";

fn board() -> (BoardWorld, FootprintLibrary) {
    let parsed = parse(BOARD);
    assert!(parsed.is_ok(), "the board parses: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let synced = sync_ast_to_world(&parsed.value, BOARD, &mut world, &mut library);
    assert!(!synced.has_errors(), "the board syncs: {:?}", synced.errors);
    (world, library)
}

/// The same board with the part turned a quarter turn.
const TURNED: &str = "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\n\
     footprint WIDE_PAD {\n    courtyard 3mm x 2mm\n    \
     pad 1 rect at 0mm, 0mm size 2mm x 0.6mm\n}\n\n\
     net SIG {\n}\n\ncomponent U1 connector \"WIDE_PAD\" {\n    at 10mm, 10mm\n    \
     rotate 90\n    pin.1 = SIG\n}\n";

/// How many cells the grid marks as pad copper on the top layer.
fn pad_cells(shape: Option<u16>) -> usize {
    let (mut world, library) = board();
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("a preset"));
    let grid = RoutingGrid::from_board_with_pads(&mut world, &library, &rules, 250_000, shape)
        .expect("a grid");

    let mut marked = 0;
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            if grid.pad_owner(x, y, 0).is_some() {
                marked += 1;
            }
        }
    }
    marked
}

#[test]
fn the_default_is_the_rectangle_the_sweep_chose() {
    // Not the rectangle alone - that shorts two fixtures - and not the disc,
    // which over-blocks every oblong pad. Two cells of reach is what
    // `pad_obstacle_shape_sweep` measured as better on five fixtures of six.
    assert_eq!(
        AutorouteConfig::default().pad_rect_extra_cells,
        Some(2),
        "the shape a board gets without asking is the one the sweep chose"
    );
}

#[test]
fn the_rectangle_blocks_less_than_the_disc_it_replaces() {
    // A 2.0mm by 0.6mm pad: the disc reaches 1.0mm in every direction, the
    // rectangle 0.3mm across its short side. The difference is board the
    // router was being kept off for no reason a rule states.
    let disc = pad_cells(None);
    let rect = pad_cells(Some(0));
    assert!(
        rect < disc,
        "the rectangle is the smaller shape: {rect} cells against {disc}"
    );
    // And the margin is real: asking for reach makes it grow again.
    assert!(
        pad_cells(Some(2)) > rect,
        "extra reach blocks more than none"
    );
}

#[test]
fn a_turned_pad_blocks_the_board_it_is_turned_across() {
    // The rectangle is only worth having if it follows the part. A pad turned
    // a quarter turn covers different board than one lying flat, and a grid
    // that marked the same cells either way would be a square in disguise.
    let flat = marked_cells(BOARD);
    let turned = marked_cells(TURNED);

    assert_eq!(
        flat.len(),
        turned.len(),
        "the same pad covers the same amount of board whichever way it faces"
    );
    assert_ne!(
        flat, turned,
        "but not the same board: a turned pad reaches across the other axis"
    );
}

/// Which cells the grid marks as pad copper, for a design.
fn marked_cells(source: &str) -> std::collections::BTreeSet<(u32, u32)> {
    let parsed = parse(source);
    assert!(parsed.is_ok(), "the board parses: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let synced = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(!synced.has_errors(), "the board syncs: {:?}", synced.errors);

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("a preset"));
    let grid = RoutingGrid::from_board_with_pads(&mut world, &library, &rules, 250_000, Some(0))
        .expect("a grid");

    let mut cells = std::collections::BTreeSet::new();
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            if grid.pad_owner(x, y, 0).is_some() {
                cells.insert((x, y));
            }
        }
    }
    cells
}
