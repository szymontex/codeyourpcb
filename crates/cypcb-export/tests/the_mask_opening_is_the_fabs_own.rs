//! The solder mask opening is the fabricator's number, not a constant.
//!
//! `cargo test -p cypcb-export --test the_mask_opening_is_the_fabs_own`
//!
//! The mask writer was called with `MaskPasteConfig::default()` at both mask
//! sites, and that default carries a hardcoded 0.05mm. A house asking for
//! anything else - JLCPCB's four-layer rules state 0.04mm, IPC Class 2 states
//! 0.075mm - got its openings drawn to the constant, and the files disagreed
//! with the rules the same board was checked against. Silently, because a mask
//! opening 0.01mm off looks like every other mask opening.
//!
//! No board in this repository changes today: both export presets are for
//! houses that publish 0.05mm, which is what the constant happened to be. That
//! is the same shape as the copper weight - the value is right and the reason
//! it was right was luck. The next preset added is the one this protects.
//!
//! What is checked here is the geometry: the opening drawn follows the number
//! it is given. That the number equals what the same house's design rules
//! publish is checked in `cypcb-cli`, where both tables are visible.

use cypcb_core::Nm;
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::mask::{export_soldermask, MaskPasteConfig};
use cypcb_export::gerber::Side;
use cypcb_export::presets::from_name;
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation, Value};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "masked".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );
    (world, FootprintLibrary::new())
}

/// The aperture sizes a mask file defines, in millimetres.
fn aperture_sizes(gerber: &str) -> Vec<f64> {
    gerber
        .lines()
        .filter_map(|line| line.split(",").nth(1))
        .filter_map(|rest| rest.split('X').next())
        .filter_map(|value| value.trim_end_matches("*%").parse().ok())
        .collect()
}

#[test]
fn every_export_preset_states_one() {
    // The cross-check against each house's design rules lives in `cypcb-cli`,
    // where both tables are visible; `cypcb-export` does not depend on the
    // checker and should not grow a dependency for a test. What belongs here
    // is that the field is populated at all.
    for name in ["jlcpcb", "pcbway"] {
        let preset = from_name(name).expect("the preset is there");
        assert!(
            preset.mask_expansion > Nm(0),
            "{name} states no mask expansion"
        );
    }
}

#[test]
fn the_opening_is_the_pad_plus_the_expansion_on_each_side() {
    // An 0402 pad is 0.6 x 0.5mm. At 0.05mm a side the opening is 0.7 x 0.6.
    let (mut world, library) = board();
    let config = MaskPasteConfig::default().with_mask_expansion(Nm::from_mm(0.05));
    let gerber = export_soldermask(
        &mut world,
        &library,
        Side::Top,
        &CoordinateFormat::FORMAT_MM_2_6,
        &config,
    )
    .expect("the mask exports");

    let sizes = aperture_sizes(&gerber);
    assert!(
        sizes.iter().any(|size| (size - 0.7).abs() < 1e-6),
        "no 0.7mm opening in: {sizes:?}"
    );
}

#[test]
fn a_fab_asking_for_less_gets_less() {
    // The case the constant was wrong for: JLCPCB's four-layer rules publish
    // 0.04mm, so the opening is 0.68 rather than 0.7.
    let (mut world, library) = board();
    let config = MaskPasteConfig::default().with_mask_expansion(Nm::from_mm(0.04));
    let gerber = export_soldermask(
        &mut world,
        &library,
        Side::Top,
        &CoordinateFormat::FORMAT_MM_2_6,
        &config,
    )
    .expect("the mask exports");

    let sizes = aperture_sizes(&gerber);
    assert!(
        sizes.iter().any(|size| (size - 0.68).abs() < 1e-6),
        "no 0.68mm opening in: {sizes:?}"
    );
    assert!(
        !sizes.iter().any(|size| (size - 0.7).abs() < 1e-6),
        "and no 0.7mm one: {sizes:?}"
    );
}
