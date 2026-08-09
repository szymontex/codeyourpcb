//! A poured plane keeps the distance its fabricator publishes.
//!
//! `cargo test -p cypcb-export --test the_pour_keeps_the_fabs_distance`
//!
//! `export_copper_layer`'s own doc comment has said since it was written that
//! `ExportJob` passes the fab's clearance through `export_copper_layer_with`.
//! **No caller ever did.** Every exported pour was filled with
//! `PourOptions::default()`, whose 0.3mm is deliberately generous rather than
//! published - so on a JLCPCB board, which publishes 0.254mm, every plane
//! shipped 0.046mm smaller on every edge than the house allows.
//!
//! Harmless in direction and wrong in kind: the number in the fabrication
//! files was not the number the board was checked against, and the comment
//! describing where it came from described something that did not happen.
//!
//! Measured on `examples/pour-island.cypcb` through the real command: same
//! region count, same draw count, and one region edge moving from
//! `Y19600000` to `Y19646000` - the 0.046mm.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::export_copper_layer_with;
use cypcb_export::pour::PourOptions;
use cypcb_export::presets::from_name;
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// A ground plane with one pad of a different net inside it.
fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "planed".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        2,
    );
    let gnd = world.intern_net("GND");
    world.intern_net("SIG");

    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(15.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );

    world.spawn_entity((Zone {
        bounds: Rect::new(Point::from_mm(2.0, 2.0), Point::from_mm(28.0, 18.0)),
        kind: ZoneKind::CopperPour,
        layer_mask: 0b01,
        name: Some("gnd_pour".to_string()),
        net: Some(gnd),
    },));

    (world, FootprintLibrary::new())
}

/// Every y coordinate the top copper draws, in nanometres.
fn drawn_ys(gerber: &str) -> Vec<i64> {
    gerber
        .lines()
        .filter(|line| line.contains("D01*"))
        .filter_map(|line| line.split('Y').nth(1))
        .filter_map(|rest| rest.trim_end_matches("D01*").parse().ok())
        .collect()
}

fn poured(clearance_mm: f64) -> String {
    let (mut world, library) = board();
    let options = PourOptions {
        clearance: Nm::from_mm(clearance_mm),
        ..Default::default()
    };
    export_copper_layer_with(
        &mut world,
        &library,
        Layer::TopCopper,
        &CoordinateFormat::FORMAT_MM_2_6,
        &options,
    )
    .expect("the layer exports")
}

#[test]
fn the_preset_states_what_its_fab_publishes() {
    // Both houses publish 0.254mm. The cross-check against each one's design
    // rules lives in `cypcb-cli`, where both tables are visible.
    for name in ["jlcpcb", "pcbway"] {
        let preset = from_name(name).expect("the preset is there");
        assert_eq!(
            preset.pour_clearance,
            Nm::from_mm(0.254),
            "{name} keeps a different distance"
        );
    }
}

#[test]
fn a_tighter_clearance_leaves_more_copper() {
    // The whole point: the number reaches the geometry. At 0.254mm the plane
    // comes 0.046mm closer to the foreign pad than at 0.3mm, on every edge
    // that faces one.
    let generous = drawn_ys(&poured(0.3));
    let published = drawn_ys(&poured(0.254));

    assert_eq!(
        generous.len(),
        published.len(),
        "the same plane, cut the same number of ways"
    );
    assert_ne!(
        generous, published,
        "the clearance changed and no coordinate moved"
    );

    // Every coordinate that moved, moved by the difference between the two.
    let moved: Vec<i64> = generous
        .iter()
        .zip(&published)
        .filter(|(a, b)| a != b)
        .map(|(a, b)| (a - b).abs())
        .collect();
    assert!(!moved.is_empty());
    for delta in &moved {
        assert_eq!(*delta, 46_000, "0.3mm - 0.254mm = 0.046mm: {moved:?}");
    }
}

#[test]
fn the_pour_still_clears_the_foreign_pad() {
    // The direction that matters: tightening the clearance must not put copper
    // on the pad. R1's pads sit at 14.5 and 15.5mm, 0.5mm tall, so the plane
    // may not cross y = 9.75mm - 0.254mm anywhere near them.
    let published = poured(0.254);
    let pad_top = 10_250_000; // 10mm + half of 0.5mm
    let nearest = drawn_ys(&published)
        .into_iter()
        .filter(|y| *y > 10_000_000)
        .min()
        .expect("the plane is drawn above the pad");

    assert!(
        nearest >= pad_top + 254_000,
        "the plane comes within {}nm of the pad",
        nearest - pad_top
    );
}
