//! The routing grid blocks a pad turned inside its footprint as it stands.
//!
//! `cargo test -p cypcb-autoroute --test a_pad_turned_in_its_footprint_blocks_the_grid_turned`
//!
//! A pad's own turn adds to its part's. With the pad at its footprint's
//! origin, a pad turned a quarter on a square part and a square pad on a
//! part turned a quarter are the same copper, so the grid must block the
//! same cells for both - here with each pad marked as its own rectangle.
//! The control is the square pad on a square part, which blocks other cells.

use cypcb_autoroute::grid::RoutingGrid;
use cypcb_core::{Nm, Point, Rect};
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// Which cells of the top layer are free around one 1.0 by 2.9 pad
/// at (10, 10): the part turned `part`, the pad `pad`.
fn free_cells(part: Rotation, pad: Rotation) -> Vec<bool> {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "P".into(),
        description: String::new(),
        bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
        courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".into(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.0), Nm::from_mm(2.9)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
            rotation: pad,
        }],
    });
    world.set_footprints(library.clone());
    world.spawn_component(
        RefDes::new("R1"),
        Value::new("x"),
        Position::from_mm(10.0, 10.0),
        part,
        FootprintRef::new("P"),
        NetConnections::new(),
    );
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("a known preset"));
    let grid = RoutingGrid::from_board_with_pads(
        &mut world,
        &library,
        &rules,
        Nm::from_mm(0.1).raw(),
        Some(0),
    )
    .expect("the board states a size");
    // Every 0.1 across the 6 by 6 square around the pad.
    (0..=60)
        .flat_map(|y| (0..=60).map(move |x| (x, y)))
        .map(|(x, y)| {
            let at = Point::from_mm(7.0 + f64::from(x) * 0.1, 7.0 + f64::from(y) * 0.1);
            let (gx, gy) = grid.nm_to_grid(at);
            grid.is_free(gx, gy, 0)
        })
        .collect()
}

#[test]
fn a_pad_turned_in_its_footprint_blocks_what_its_part_turned_would() {
    let turned_pad = free_cells(Rotation::ZERO, Rotation::DEG_90);
    assert!(turned_pad == free_cells(Rotation::DEG_90, Rotation::ZERO));
    assert!(
        turned_pad != free_cells(Rotation::ZERO, Rotation::ZERO),
        "the control: a turn moves the blocked cells"
    );
}
