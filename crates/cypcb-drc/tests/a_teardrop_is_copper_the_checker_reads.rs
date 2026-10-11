//! A teardrop is copper in the board, and the checker reads it as copper.
//!
//! `cargo test -p cypcb-drc --test a_teardrop_is_copper_the_checker_reads`
//!
//! Until this was written a teardrop existed only in the Gerber file: the
//! fabricator received a fillet that no rule had measured, so a fillet that
//! reached into another net's clearance went to the fab unreported. KiCad
//! makes each teardrop a zone on the track's layer and net (`createTeardrop`
//! in `pcbnew/teardrop/teardrop.cpp`), and a zone is checked like any copper.
//!
//! The board here is one round land 1.6mm across with a 0.2mm track leaving
//! it to the right. At the board's ordinary ratios the fillet is 0.72mm
//! half-width where it leaves the land and runs out to the track's width 1.6mm
//! from the land's centre. A second track runs beside it, 0.4mm above the
//! track's own edge: clear of the track and of the land, and a hair inside the
//! checker's distance from the fillet's flank.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{
    copper_pieces, ClearanceRule, DrcRule, EdgeClearanceRule, NetSplitRule, PadEntryRule,
    UnroutedPinRule,
};
use cypcb_drc::{shorts, ViolationKind};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position, RefDes,
    Rotation, Teardrops, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::teardrop::{teardrop_copper, TeardropCopper};
use cypcb_world::BoardWorld;

/// One round SMD land, 1.6mm across, on the top layer.
fn land() -> Footprint {
    let size = Nm::from_mm(1.6);
    Footprint {
        name: "LAND".to_string(),
        description: "one round land".to_string(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (size, size),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        }],
        bounds: Rect::from_center_size(Point::ORIGIN, (size, size)),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.4), Nm::from_mm(2.4))),
        silk: Vec::new(),
    }
}

fn place(world: &mut BoardWorld, refdes: &str, at: (f64, f64), net: NetId) {
    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1", net));
    world.spawn_component(
        RefDes::new(refdes),
        Value::new("x"),
        Position::from_mm(at.0, at.1),
        Rotation::ZERO,
        FootprintRef::new("LAND"),
        nets,
    );
}

fn track(world: &mut BoardWorld, net: NetId, width: f64, points: &[(f64, f64)]) {
    let segments = points
        .windows(2)
        .map(|pair| {
            TraceSegment::new(
                Point::from_mm(pair[0].0, pair[0].1),
                Point::from_mm(pair[1].0, pair[1].1),
            )
        })
        .collect();
    world.spawn_entity((
        Trace {
            segments,
            width: Nm::from_mm(width),
            layer: Layer::TopCopper,
            net_id: net,
            locked: false,
            source: TraceSource::Manual,
        },
        net,
    ));
}

/// A board with the land at (10, 10) on net A, a track leaving it to the
/// right, and a second track of net `other` beside the track's flank.
/// `asked` says whether the board asks for teardrops.
fn board(asked: bool, other_is_a: bool) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(land());
    world.set_footprints(library.clone());

    let a = world.intern_net("A");
    let b = world.intern_net("B");
    place(&mut world, "R1", (10.0, 10.0), a);
    track(&mut world, a, 0.2, &[(10.0, 10.0), (16.0, 10.0)]);
    // 0.52mm above the centreline at its lower edge: 0.42mm clear of the
    // track and 0.33mm clear of the land, and under the 0.127mm of the preset
    // from the fillet's flank at 11mm.
    track(
        &mut world,
        if other_is_a { a } else { b },
        0.2,
        &[(11.0, 10.62), (13.0, 10.62)],
    );
    if asked {
        assert!(world.set_teardrops(Teardrops::default()));
    }
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

fn rules() -> DesignRules {
    DesignRules::jlcpcb_2layer()
}

#[test]
fn the_board_that_asks_holds_the_fillet_as_copper_on_the_tracks_net() {
    let (mut world, _) = board(true, false);
    let held = teardrop_copper(&mut world);
    assert_eq!(held.len(), 1, "one track end lands in the land: {held:?}");
    let (_, copper, net) = &held[0];
    assert_eq!(copper.layer, Layer::TopCopper, "the track's layer");
    assert_eq!(Some(*net), world.get_net("A"), "the track's net");
    assert_eq!(copper.outline.len(), 4);
}

#[test]
fn the_board_that_never_asked_holds_none() {
    let (mut world, _) = board(false, false);
    assert!(teardrop_copper(&mut world).is_empty());
}

#[test]
fn a_fillet_that_reaches_into_another_nets_clearance_is_reported() {
    // The positive control. The same two tracks, with and without the fillet:
    // without it every gap is wide enough, with it the flank is inside the
    // preset's distance of the second track.
    let (mut without, _) = board(false, false);
    let before = ClearanceRule.check(&mut without, &rules());
    assert!(before.is_empty(), "no fillet, no violation: {before:?}");

    let (mut with, _) = board(true, false);
    let after = ClearanceRule.check(&mut with, &rules());
    assert_eq!(after.len(), 1, "the fillet is one place: {after:?}");
    let row = &after[0];
    assert_eq!(row.kind, ViolationKind::Clearance);
    assert!(
        row.actual.expect("a distance") < rules().min_clearance,
        "{row:?}"
    );
    assert!(
        row.actual.expect("a distance") > Nm::ZERO,
        "close, not touching: {row:?}"
    );
    assert_eq!(shorts(&after), 0);

    // The row a person reads names the fillet and its net.
    let named: Vec<String> = cypcb_drc::run_drc(&mut with, &rules())
        .violations
        .into_iter()
        .filter(|row| row.kind == ViolationKind::Clearance)
        .map(|row| row.message)
        .collect();
    assert_eq!(named.len(), 1, "{named:?}");
    assert!(
        named[0].contains("teardrop 'A'") && named[0].contains("trace 'B'"),
        "{named:?}"
    );
}

#[test]
fn a_fillet_touching_its_own_nets_copper_is_not_reported() {
    // The same geometry, the second track on the fillet's own net.
    let (mut world, _) = board(true, true);
    let found = ClearanceRule.check(&mut world, &rules());
    assert!(found.is_empty(), "same net is a join, not a gap: {found:?}");
}

#[test]
fn a_track_on_another_net_that_ends_on_the_land_is_still_a_short() {
    // The land is on net A, the track on B. The track already shorts the land;
    // the fillet takes the track's net and does not change where it is.
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(land());
    world.set_footprints(library.clone());
    let a = world.intern_net("A");
    let b = world.intern_net("B");
    place(&mut world, "R1", (10.0, 10.0), a);
    track(&mut world, b, 0.2, &[(10.0, 10.0), (16.0, 10.0)]);
    assert!(world.set_teardrops(Teardrops::default()));
    world.rebuild_spatial_index_from_library(&library);

    let held = teardrop_copper(&mut world);
    assert_eq!(held.len(), 1);
    assert_eq!(
        Some(held[0].2),
        world.get_net("B"),
        "the track's net, not the land's"
    );

    let found = ClearanceRule.check(&mut world, &rules());
    assert!(
        shorts(&found) >= 1,
        "copper of B on the land of A is a short: {found:?}"
    );
    // One place is one row: the fillet is copper of the same net on the same
    // land as the track, so it adds no second line for the one short.
    assert_eq!(shorts(&found), 1, "{found:?}");

    // And the geometry is the geometry of the matching case.
    let (mut matching, _) = board(true, false);
    assert_eq!(
        teardrop_copper(&mut matching)[0].1.outline,
        held[0].1.outline,
        "the polygon does not depend on the nets"
    );
}

/// Two lands of net A (R1 at 10, R2 at 18), a stub of the net from R1 that
/// stops short of the track from R2, with the fillet between them.
fn net_a_in_two_pieces(asked: bool) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(land());
    world.set_footprints(library.clone());
    let a = world.intern_net("A");
    place(&mut world, "R1", (10.0, 10.0), a);
    place(&mut world, "R2", (18.0, 10.0), a);
    // The stub ends 1.0mm from R1's centre. The fillet runs to 1.6mm. The
    // other track starts at 11.4mm: 0.4mm from the stub's end, a gap of 0.2mm
    // between the copper of the two, and inside the fillet.
    track(&mut world, a, 0.2, &[(10.0, 10.0), (11.0, 10.0)]);
    track(&mut world, a, 0.2, &[(11.4, 10.0), (18.0, 10.0)]);
    if asked {
        assert!(world.set_teardrops(Teardrops::default()));
    }
    world.rebuild_spatial_index_from_library(&library);
    world
}

#[test]
fn copper_joined_only_by_a_fillet_is_one_piece() {
    let mut without = net_a_in_two_pieces(false);
    let a = without.get_net("A").expect("net A");
    let pieces = |world: &mut BoardWorld| {
        copper_pieces(world)
            .into_iter()
            .find(|net| net.net == a)
            .expect("net A has copper")
            .pieces_with_pads()
            .count()
    };
    assert_eq!(pieces(&mut without), 2, "control: the gap splits the net");
    assert!(
        !NetSplitRule.check(&mut without, &rules()).is_empty(),
        "control: the split is reported"
    );

    let mut with = net_a_in_two_pieces(true);
    assert_eq!(pieces(&mut with), 1, "the fillet closes the gap");
    assert!(
        NetSplitRule.check(&mut with, &rules()).is_empty(),
        "and the net is no longer reported as split"
    );
    assert!(UnroutedPinRule.check(&mut with, &rules()).is_empty());
}

#[test]
fn a_board_that_asks_has_the_teardrops_its_tracks_give() {
    let (mut world, _) = board(true, false);
    assert!(
        PadEntryRule
            .check(&mut world, &rules())
            .iter()
            .all(|row| !row.message.contains("teardrop")),
        "the copper the board holds is the copper its tracks give"
    );
}

#[test]
fn control_a_fillet_the_board_lacks_is_reported() {
    let (mut world, _) = board(true, false);
    let entities: Vec<_> = teardrop_copper(&mut world)
        .into_iter()
        .map(|(entity, _, _)| entity)
        .collect();
    for entity in entities {
        world.ecs_mut().despawn(entity);
    }
    let found = PadEntryRule.check(&mut world, &rules());
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].kind, ViolationKind::PadEntry);
    assert!(
        found[0].message.starts_with("R1.1:"),
        "{}",
        found[0].message
    );
    assert!(found[0].message.contains("none is in the copper"));
}

#[test]
fn control_a_fillet_no_track_gives_is_reported() {
    let (mut world, _) = board(true, false);
    world.spawn_entity((
        TeardropCopper {
            outline: vec![
                Point::from_mm(2.0, 2.0),
                Point::from_mm(3.0, 2.0),
                Point::from_mm(3.0, 3.0),
            ],
            layer: Layer::TopCopper,
            order: 99,
            land: None,
        },
        NetId::new(0),
    ));
    let found = PadEntryRule.check(&mut world, &rules());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].message.contains("no track meets the land"));
}

#[test]
fn a_board_that_never_asked_is_not_asked_to_account_for_any() {
    let (mut world, _) = board(false, false);
    assert!(PadEntryRule.check(&mut world, &rules()).is_empty());
}

/// Two lands of net A, 2.3mm apart, and a stub of the net from the first that
/// ends 0.6mm from its centre: 1.7mm short of the second land's edge. The
/// fillet runs to 1.6mm from the first centre, into the second land.
fn a_land_only_the_fillet_reaches(asked: bool) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(land());
    world.set_footprints(library.clone());
    let a = world.intern_net("A");
    place(&mut world, "R1", (10.0, 10.0), a);
    place(&mut world, "R2", (12.3, 10.0), a);
    track(&mut world, a, 0.2, &[(10.0, 10.0), (10.6, 10.0)]);
    if asked {
        assert!(world.set_teardrops(Teardrops::default()));
    }
    world.rebuild_spatial_index_from_library(&library);
    world
}

#[test]
fn a_land_the_fillet_reaches_is_reached() {
    let mut without = a_land_only_the_fillet_reaches(false);
    let unreached = UnroutedPinRule.check(&mut without, &rules());
    assert_eq!(
        unreached.len(),
        1,
        "control: nothing reaches R2: {unreached:?}"
    );

    let mut with = a_land_only_the_fillet_reaches(true);
    let unreached = UnroutedPinRule.check(&mut with, &rules());
    assert!(
        unreached.is_empty(),
        "the fillet reaches into R2's land: {unreached:?}"
    );

    // And the net is one piece of copper with the fillet, two without it.
    let pieces = |world: &mut BoardWorld| {
        let a = world.get_net("A").expect("net A");
        copper_pieces(world)
            .into_iter()
            .find(|net| net.net == a)
            .expect("net A has copper")
            .pieces_with_pads()
            .count()
    };
    assert_eq!(pieces(&mut without), 2);
    assert_eq!(pieces(&mut with), 1);
}

/// A land 1.7mm from the left edge, a track from its centre to 0.95mm from the
/// edge, and so a fillet that runs on past the track's end to 0.1mm from the
/// edge. The land and the track keep their distance.
fn a_fillet_toward_the_edge(asked: bool) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(land());
    world.set_footprints(library.clone());
    let a = world.intern_net("A");
    place(&mut world, "R1", (1.7, 10.0), a);
    track(&mut world, a, 0.2, &[(1.7, 10.0), (0.95, 10.0)]);
    if asked {
        assert!(world.set_teardrops(Teardrops::default()));
    }
    world.rebuild_spatial_index_from_library(&library);
    world
}

#[test]
fn a_fillet_too_near_the_edge_is_reported() {
    let mut without = a_fillet_toward_the_edge(false);
    let found = EdgeClearanceRule.check(&mut without, &rules());
    assert!(
        found.is_empty(),
        "control: land and track keep away: {found:?}"
    );

    let mut with = a_fillet_toward_the_edge(true);
    let found = EdgeClearanceRule.check(&mut with, &rules());
    assert_eq!(
        found.len(),
        1,
        "the fillet is 0.1mm from the edge: {found:?}"
    );
    assert!(
        found[0].actual.expect("a distance") < rules().min_edge_clearance,
        "{found:?}"
    );
}

#[test]
fn rebuilding_leaves_the_same_copper_and_the_same_order() {
    // A rebuild takes the old copper out and puts the tracks' copper in. The
    // entities it hands out are indices freed a moment before, which come back
    // in the reverse order, so the order a file is written in is the builder's
    // and not the entities'.
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(land());
    world.set_footprints(library.clone());
    let a = world.intern_net("A");
    place(&mut world, "R1", (10.0, 10.0), a);
    place(&mut world, "R2", (18.0, 10.0), a);
    track(&mut world, a, 0.2, &[(10.0, 10.0), (18.0, 10.0)]);
    assert!(world.set_teardrops(Teardrops::default()));

    let outlines = |world: &mut BoardWorld| -> Vec<Vec<Point>> {
        teardrop_copper(world)
            .into_iter()
            .map(|(_, copper, _)| copper.outline)
            .collect()
    };
    world.rebuild_spatial_index_from_library(&library);
    let first = outlines(&mut world);
    assert_eq!(first.len(), 2, "one at each end of the track");
    for _ in 0..3 {
        world.rebuild_spatial_index_from_library(&library);
        assert_eq!(outlines(&mut world), first);
    }
}

#[test]
fn a_track_passing_the_tip_of_a_fillet_is_measured_along_its_length() {
    // The stub ends 0.9mm from the land's centre, and the fillet runs on to
    // 1.6mm. A second track crosses beyond that tip, 0.05mm from it, and its
    // closest point to the tip is in the middle of its length, not at an end.
    let build = |asked: bool| {
        let mut world = BoardWorld::new();
        world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
        let mut library = FootprintLibrary::new();
        library.register(land());
        world.set_footprints(library.clone());
        let a = world.intern_net("A");
        let b = world.intern_net("B");
        place(&mut world, "R1", (10.0, 10.0), a);
        track(&mut world, a, 0.2, &[(10.0, 10.0), (10.9, 10.0)]);
        track(&mut world, b, 0.2, &[(11.75, 9.0), (11.75, 11.0)]);
        if asked {
            assert!(world.set_teardrops(Teardrops::default()));
        }
        world.rebuild_spatial_index_from_library(&library);
        world
    };
    let found = ClearanceRule.check(&mut build(false), &rules());
    assert!(
        found.is_empty(),
        "control: without the fillet every gap is wide: {found:?}"
    );
    let found = ClearanceRule.check(&mut build(true), &rules());
    assert_eq!(
        found.len(),
        1,
        "the tip is 0.05mm from the track: {found:?}"
    );
}
