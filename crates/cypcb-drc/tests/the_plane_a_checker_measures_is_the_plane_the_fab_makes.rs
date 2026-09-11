//! The plane a checker measures is the plane the fabricator will make.
//!
//! `cargo test -p cypcb-drc --test the_plane_a_checker_measures_is_the_plane_the_fab_makes`
//!
//! `PourIslandRule` fills every pour itself, because an island is a property
//! of the copper rather than of the outline. It used to fill from
//! `PourOptions::default()` for everything but the clearance, so the relief it
//! cut around a pad of the pour's own net was the crate's generous 0.254mm
//! whatever the house published. JLCPCB's advanced process publishes 0.2mm.
//!
//! The two numbers are 0.054mm apart, which is not a rounding difference: it
//! is 0.054mm of copper, on every side of every own-net pad, either present or
//! absent in the plane the rule then looks for islands in. This test measures
//! that the difference reaches the copper, and the unit test beside the rule
//! measures that the rule asks the house rather than the default.

use cypcb_core::pour::PourOptions;
use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::{DesignRules, Preset, PresetRules};
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, Position, RefDes, Rotation, Value};

/// A ground pour with one ground pad in the middle of it.
fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
    let gnd = world.intern_net("GND");

    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "PAD1".into(),
        description: String::new(),
        bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
        courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".into(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        }],
    });
    world.set_footprints(library);

    let mut connections = NetConnections::new();
    connections.add(PinConnection::new("1".to_string(), gnd));
    world.spawn_component(
        RefDes::new("J1"),
        Value::new(""),
        Position(Point::from_mm(20.0, 20.0)),
        Rotation::ZERO,
        FootprintRef::new("PAD1"),
        connections,
    );

    world.spawn_entity(Zone {
        bounds: Rect {
            min: Point::from_mm(5.0, 5.0),
            max: Point::from_mm(35.0, 35.0),
        },
        kind: ZoneKind::CopperPour,
        layer_mask: Layer::TopCopper.to_copper_mask(),
        name: Some("gnd".to_string()),
        net: Some(gnd),
    });

    world.rebuild_spatial_index_from_library(&world.footprints().clone());
    world
}

/// The options the rule builds, spelled out here because the rule's own copy
/// is private: the unit test in `pour_island.rs` is what holds those two
/// honest to each other.
fn options(rules: &DesignRules) -> PourOptions {
    PourOptions {
        clearance: rules.min_clearance,
        thermal_gap: rules.thermal_relief_gap,
        spoke_width: rules.thermal_relief_spoke_width,
    }
}

/// Every rectangle of copper the pour becomes, in nanometres.
fn copper(rules: &DesignRules) -> Vec<[i64; 4]> {
    let mut world = board();
    let library = world.footprints().clone();
    let (_, zone) = world
        .zones()
        .into_iter()
        .find(|(_, zone)| zone.kind == ZoneKind::CopperPour)
        .expect("the pour is there");

    let filled = cypcb_world::copper::fill_zone(
        &mut world,
        &library,
        Layer::TopCopper,
        &zone,
        None,
        &options(rules),
    );
    filled
        .all()
        .map(|r| [r.min.x.0, r.min.y.0, r.max.x.0, r.max.y.0])
        .collect()
}

#[test]
fn two_houses_with_different_reliefs_get_different_copper() {
    let standard = copper(&Preset::JlcpcbStandard2Layer.rules());
    let advanced = copper(&Preset::JlcpcbAdvanced2Layer.rules());

    // The control: the same house twice is the same plane, so a difference
    // below can only have come from the numbers that differ.
    assert_eq!(
        standard,
        copper(&Preset::JlcpcbStandard2Layer.rules()),
        "the same preset filled the same board two different ways"
    );

    assert_ne!(
        standard, advanced,
        "0.254mm of relief and 0.2mm of relief cut the same plane"
    );

    // And the difference is the relief itself: the keepout around a 1mm pad
    // centred at 20mm reaches 20.754mm on the standard process and 20.7mm on
    // the advanced one, so the copper starts there.
    let edge = |plane: &[[i64; 4]]| {
        plane
            .iter()
            .filter(|r| r[0] > 20_000_000)
            .map(|r| r[0])
            .min()
            .expect("copper to the right of the pad")
    };
    assert_eq!(edge(&standard), 20_754_000);
    assert_eq!(edge(&advanced), 20_700_000);
}
