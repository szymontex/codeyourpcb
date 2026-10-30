//! The fabrication files cut a pour away from the rule areas that forbid it.
//!
//! `cargo test -p cypcb-export --test the_fabrication_files_cut_the_pour_away_from_a_rule_area`
//!
//! The viewer and the exporters share one fill, so a rule area that forbids
//! copper pour takes its square out of the pour in the Gerber and in the
//! IPC-2581 file alike. Hand check: a 26 x 16mm plane from (2, 2) to (28, 18)
//! with a rule area from (10, 5) to (14, 9); the Gerber has regions whose edges
//! stand on x = 10 and x = 14 and y = 5 and y = 9, and none crosses them.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::export_copper_layer_with;
use cypcb_export::ipc2581::{export_ipc2581, HouseTolerances};
use cypcb_export::pour::PourOptions;
use cypcb_world::components::zone::{KeepoutKind, KeepoutKinds, Zone, ZoneKind};
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

fn board(area: Option<&[KeepoutKind]>) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let gnd = world.intern_net("GND");
    world.spawn_entity((Zone {
        bounds: Rect::new(Point::from_mm(2.0, 2.0), Point::from_mm(28.0, 18.0)),
        kind: ZoneKind::CopperPour,
        layer_mask: 0b01,
        name: Some("gnd_pour".to_string()),
        net: Some(gnd),
    },));
    let zone = Zone::keepout(
        Rect::new(Point::from_mm(10.0, 5.0), Point::from_mm(14.0, 9.0)),
        0b01,
    );
    match area {
        Some(kinds) => world.spawn_entity((zone, KeepoutKinds::of_kinds(kinds))),
        None => world.spawn_entity((zone,)),
    };
    world
}

fn gerber(world: &mut BoardWorld) -> String {
    export_copper_layer_with(
        world,
        &FootprintLibrary::new(),
        Layer::TopCopper,
        &CoordinateFormat::FORMAT_MM_2_6,
        &PourOptions::default(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .expect("the layer exports")
}

/// The x and y of every corner the Gerber draws, in millimetres.
fn corners(gerber: &str) -> Vec<(f64, f64)> {
    gerber
        .lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('*');
            let (x, rest) = line.strip_prefix('X')?.split_once('Y')?;
            let y: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            Some((x.parse::<f64>().ok()? / 1e6, y.parse::<f64>().ok()? / 1e6))
        })
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn the_gerber_has_corners_on_the_area_and_none_inside_it() {
    let mut world = board(Some(&[KeepoutKind::Pour]));
    let corners = corners(&gerber(&mut world));
    assert!(
        !corners.is_empty(),
        "no region corner read; the format moved?"
    );
    for (x, y) in &corners {
        let inside = *x > 10.0 + 1e-6 && *x < 14.0 - 1e-6 && *y > 5.0 + 1e-6 && *y < 9.0 - 1e-6;
        assert!(!inside, "a corner inside the area: ({x}, {y})");
    }
    // Hand-checkable vertices: the four corners of the area are corners of the
    // copper round it, with no gap.
    for (x, y) in [(10.0, 5.0), (14.0, 5.0), (14.0, 9.0), (10.0, 9.0)] {
        assert!(
            corners.iter().any(|c| near(c.0, x) && near(c.1, y)),
            "no corner at ({x}, {y})"
        );
    }
}

#[test]
fn an_area_that_forbids_only_tracks_leaves_the_gerber_as_it_was() {
    let mut cut = board(Some(&[KeepoutKind::Tracks]));
    let mut none = BoardWorld::new();
    none.set_board("b".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let gnd = none.intern_net("GND");
    none.spawn_entity((Zone {
        bounds: Rect::new(Point::from_mm(2.0, 2.0), Point::from_mm(28.0, 18.0)),
        kind: ZoneKind::CopperPour,
        layer_mask: 0b01,
        name: Some("gnd_pour".to_string()),
        net: Some(gnd),
    },));
    assert_eq!(gerber(&mut cut), gerber(&mut none));
}

#[test]
fn the_five_kind_keepout_cuts_the_gerber_as_well() {
    let mut a = board(None);
    let mut b = board(Some(&[KeepoutKind::Pour]));
    assert_eq!(gerber(&mut a), gerber(&mut b));
}

#[test]
fn the_ipc2581_file_is_cut_the_same_way() {
    let plain = |world: &mut BoardWorld| {
        export_ipc2581(
            world,
            &FootprintLibrary::new(),
            HouseTolerances::default(),
            cypcb_export::stamp::Stamp::UNIX_EPOCH,
        )
        .0
    };
    let cut = plain(&mut board(Some(&[KeepoutKind::Pour])));
    let whole = plain(&mut board(Some(&[KeepoutKind::Tracks])));
    assert_ne!(
        cut, whole,
        "the area takes nothing out of the IPC-2581 pour"
    );
    // The area's own corner is a vertex of the copper that is left.
    assert!(
        cut.contains("<PolyBegin x=\"10.000\" y=\"5.000\"/>")
            || cut.contains("<PolyStepSegment x=\"10.000\" y=\"5.000\"/>"),
        "{cut}"
    );
}
