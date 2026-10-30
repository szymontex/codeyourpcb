//! A zone on several layers is one zone in the file KiCad reads.
//!
//! `cargo test -p cypcb-kicad --test a_zone_on_several_layers_is_one_zone_in_the_file`
//!
//! KiCad keeps a zone on several layers as one object and writes it as one
//! `(zone ...)` with `(layers "F.Cu" "B.Cu" ...)`: `format( const ZONE* )` in
//! `pcb_io_kicad_sexpr.cpp` enumerates the layers of a zone on copper, so the
//! set names `F&B.Cu` and `*.Cu` do not appear. This writer used to loop over
//! the layers and write a zone for each. Measured on the 132 boards of this
//! machine's KiCad trees, 73 zones with a mask of two to six layers came back
//! from a trip as 200, on 27 boards. Written and read again, the board has
//! `n` zones where the design had one.
//!
//! The boards here are written by hand, so they are evidence about this
//! writer, not about what KiCad emits.

use cypcb_core::{Nm, Point, Rect};
use cypcb_kicad::board_writer::write_board;
use cypcb_kicad::pcb_parser::parse_kicad_pcb_str;
use cypcb_world::components::zone::{KeepoutKind, KeepoutKinds, Zone, ZoneKind};
use cypcb_world::BoardWorld;

const COPPER: u8 = 4;

fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "layered".to_string(),
        (Nm::from_mm(40.0), Nm::from_mm(30.0)),
        COPPER,
    );
    world.intern_net("GND");
    world
}

fn bounds() -> Rect {
    Rect::new(Point::from_mm(2.0, 2.0), Point::from_mm(38.0, 28.0))
}

fn pour(world: &mut BoardWorld, mask: u32) {
    let gnd = world.intern_net("GND");
    world.spawn_entity((Zone {
        bounds: bounds(),
        kind: ZoneKind::CopperPour,
        layer_mask: mask,
        name: Some("gnd_pour".to_string()),
        net: Some(gnd),
    },));
}

fn keepout(world: &mut BoardWorld, mask: u32) {
    world.spawn_entity((Zone {
        bounds: bounds(),
        kind: ZoneKind::Keepout,
        layer_mask: mask,
        name: None,
        net: None,
    },));
}

/// Kind, mask, bounds and net of every zone, in a fixed order.
fn zones(world: &mut BoardWorld) -> Vec<(String, u32, Rect, Option<u32>)> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Zone>();
    let mut all: Vec<_> = query
        .iter(ecs)
        .map(|z| {
            (
                format!("{:?}", z.kind),
                z.layer_mask,
                z.bounds,
                z.net.map(|n| n.0),
            )
        })
        .collect();
    all.sort_by_key(|(kind, mask, bounds, _)| {
        (kind.clone(), *mask, bounds.min.x.0, bounds.min.y.0)
    });
    all
}

fn read_back(text: &str) -> BoardWorld {
    parse_kicad_pcb_str(text)
        .expect("this project can read its own output")
        .world
}

/// The `(zone` blocks of a written board, one string each.
fn zone_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if line == "  (zone" {
            current = Some(String::new());
        }
        if let Some(block) = current.as_mut() {
            block.push_str(line);
            block.push('\n');
        }
        if line == "  )" {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
        }
    }
    blocks
}

/// Every mask of two or more of the four copper layers.
fn masks_of_several() -> Vec<u32> {
    (0u32..16).filter(|m| m.count_ones() >= 2).collect()
}

#[test]
fn a_pour_on_several_layers_is_written_as_one_zone() {
    for mask in masks_of_several() {
        let mut world = board();
        pour(&mut world, mask);
        let text = write_board(&mut world, "cypcb");
        let blocks = zone_blocks(&text);
        assert_eq!(blocks.len(), 1, "mask {mask:#06b}: {text}");
        assert_eq!(blocks[0].matches("(layers ").count(), 1, "mask {mask:#06b}");
        assert!(!blocks[0].contains("(layer \""), "mask {mask:#06b}");
    }
}

#[test]
fn a_keepout_on_several_layers_is_written_as_one_zone() {
    for mask in masks_of_several() {
        let mut world = board();
        keepout(&mut world, mask);
        let text = write_board(&mut world, "cypcb");
        let blocks = zone_blocks(&text);
        assert_eq!(blocks.len(), 1, "mask {mask:#06b}: {text}");
        assert!(blocks[0].contains("(layers "), "mask {mask:#06b}");
        assert!(blocks[0].contains("(keepout "), "mask {mask:#06b}");
    }
}

#[test]
fn the_layers_are_listed_from_the_top_and_never_named_as_a_set() {
    // KiCad lists a copper zone's layers by layer number: F.Cu, B.Cu, In1.Cu...
    let mut world = board();
    pour(&mut world, 0b1111);
    let text = write_board(&mut world, "cypcb");
    let block = &zone_blocks(&text)[0];
    assert!(
        block.contains("(layers \"F.Cu\" \"B.Cu\" \"In1.Cu\" \"In2.Cu\")"),
        "{block}"
    );
    for set in ["F&B.Cu", "*.Cu", "*In.Cu"] {
        assert!(!text.contains(set), "{set} in {text}");
    }
}

#[test]
fn a_zone_on_several_layers_comes_back_as_the_same_zone() {
    for mask in masks_of_several() {
        let mut world = board();
        pour(&mut world, mask);
        keepout(&mut world, mask);
        let before = zones(&mut world);
        let text = write_board(&mut world, "cypcb");
        let mut back = read_back(&text);
        assert_eq!(zones(&mut back), before, "mask {mask:#06b}");
        assert_eq!(before.len(), 2);
    }
}

#[test]
fn a_zone_on_one_layer_is_written_as_it_always_was() {
    for (mask, layer) in [
        (0b0001, "F.Cu"),
        (0b0010, "B.Cu"),
        (0b0100, "In1.Cu"),
        (0b1000, "In2.Cu"),
    ] {
        let mut world = board();
        pour(&mut world, mask);
        let text = write_board(&mut world, "cypcb");
        let block = &zone_blocks(&text)[0];
        assert!(
            block.contains(&format!("    (layer \"{layer}\")\n")),
            "{block}"
        );
        assert!(!block.contains("(layers "), "{block}");
    }
}

#[test]
fn a_zone_on_no_copper_layer_writes_nothing() {
    let mut world = board();
    pour(&mut world, 0);
    keepout(&mut world, 0);
    let text = write_board(&mut world, "cypcb");
    assert!(zone_blocks(&text).is_empty(), "{text}");
}

#[test]
fn what_a_rule_area_forbids_is_kept_when_it_spans_layers() {
    let mut world = board();
    world.spawn_entity((
        Zone {
            bounds: bounds(),
            kind: ZoneKind::Keepout,
            layer_mask: 0b0111,
            name: None,
            net: None,
        },
        KeepoutKinds::of_kinds(&[KeepoutKind::Pour, KeepoutKind::Vias]),
    ));
    let text = write_board(&mut world, "cypcb");
    let blocks = zone_blocks(&text);
    assert_eq!(blocks.len(), 1, "{text}");
    assert!(
        blocks[0].contains(
            "(keepout (tracks allowed) (vias not_allowed) (pads allowed) (copperpour not_allowed) (footprints allowed))"
        ),
        "{}",
        blocks[0]
    );
    let mut back = read_back(&text);
    let ecs = back.ecs_mut();
    let mut query = ecs.query::<(&Zone, Option<&KeepoutKinds>)>();
    let rows: Vec<_> = query.iter(ecs).collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0.layer_mask, 0b0111);
    assert_eq!(
        rows[0].1.copied(),
        Some(KeepoutKinds::of_kinds(&[
            KeepoutKind::Pour,
            KeepoutKind::Vias
        ]))
    );
}

#[test]
fn the_zones_stay_in_the_order_they_were_built_when_one_spans_layers() {
    let mut world = board();
    pour(&mut world, 0b0001);
    pour(&mut world, 0b0110);
    keepout(&mut world, 0b1001);
    let text = write_board(&mut world, "cypcb");
    let layers: Vec<String> = zone_blocks(&text)
        .iter()
        .map(|b| {
            b.lines()
                .find(|l| l.trim_start().starts_with("(layer"))
                .unwrap()
                .trim()
                .to_string()
        })
        .collect();
    assert_eq!(
        layers,
        [
            "(layer \"F.Cu\")",
            "(layers \"B.Cu\" \"In1.Cu\")",
            "(layers \"F.Cu\" \"In2.Cu\")"
        ]
    );
}

/// Every `.kicad_pcb` this repository holds, written and read again: each zone
/// has the kind, mask, bounds and net it went in with. The fixtures hold no
/// zone on several layers, so this is the check that one layer stays as it was
/// on real boards; the hand-built boards above hold the several.
#[test]
fn every_board_the_repository_holds_keeps_its_zones_through_a_trip() {
    let root = cypcb_fixtures::tree::repo_root();
    let boards: Vec<String> = cypcb_fixtures::tree::files_git_tracks(&root)
        .into_iter()
        .filter(|f| f.ends_with(".kicad_pcb"))
        .collect();
    assert!(!boards.is_empty());
    let mut with_zones = 0;
    for board in &boards {
        let text = std::fs::read_to_string(root.join(board)).expect("a tracked board reads");
        let Ok(parsed) = parse_kicad_pcb_str(&text) else {
            continue;
        };
        let mut world = parsed.world;
        let before = zones(&mut world);
        if !before.is_empty() {
            with_zones += 1;
        }
        let written = write_board(&mut world, "cypcb");
        let mut back = read_back(&written);
        assert_eq!(zones(&mut back), before, "{board}");
    }
    assert!(
        with_zones > 0,
        "no fixture holds a zone, so this checks nothing"
    );
}
