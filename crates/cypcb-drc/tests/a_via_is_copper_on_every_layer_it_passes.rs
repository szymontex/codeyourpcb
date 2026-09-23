//! A via is copper on every layer it is drilled through.
//!
//! `cargo test -p cypcb-drc --test a_via_is_copper_on_every_layer_it_passes`
//!
//! Until 2026-09-24 the spatial index gave a via the layers it joins and no
//! others, so a Top-to-Inner2 via was invisible on Inner1 and a through via on
//! both inner layers of a four-layer board. The hole is plated wherever it is
//! drilled, and the Gerber export flashes the via's land on each of those
//! layers, so another net's track run across it on Inner1 is copper on copper
//! that no clearance check ever saw.

use cypcb_core::{Nm, Point};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{ClearanceRule, DrcRule};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::components::{Layer, NetId};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// A four-layer board with one via of net 1 at 10mm, 10mm and a track of net 2
/// on `track_layer` running straight across it.
fn board(span: (Layer, Layer), track_layer: Layer) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 4);

    let via_net = NetId::new(1);
    world.spawn_entity((
        Via {
            position: Point::from_mm(10.0, 10.0),
            drill: Nm::from_mm(0.3),
            outer_diameter: Nm::from_mm(0.6),
            start_layer: span.0,
            end_layer: span.1,
            net_id: via_net,
            locked: false,
        },
        via_net,
    ));

    let track_net = NetId::new(2);
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(5.0, 10.0),
                Point::from_mm(15.0, 10.0),
            )],
            width: Nm::from_mm(0.2),
            layer: track_layer,
            net_id: track_net,
            locked: false,
            source: TraceSource::Manual,
        },
        track_net,
    ));

    let library = FootprintLibrary::new();
    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);
    world
}

fn reports(span: (Layer, Layer), track_layer: Layer) -> usize {
    ClearanceRule
        .check(&mut board(span, track_layer), &DesignRules::default())
        .len()
}

#[test]
fn a_blind_via_meets_the_track_on_the_layer_it_passes() {
    let blind = (Layer::TopCopper, Layer::Inner(1));
    assert_eq!(reports(blind, Layer::Inner(0)), 1, "the layer it passes");
    // The layer it joins, reported before and after: the control that the
    // board is built the way the first case assumes.
    assert_eq!(reports(blind, Layer::Inner(1)), 1, "the layer it joins");
    // A blind via stops: the far face is not its copper.
    assert_eq!(
        reports(blind, Layer::BottomCopper),
        0,
        "the face it stops short of"
    );
}

#[test]
fn a_through_via_meets_the_track_on_every_inner_layer() {
    let through = (Layer::TopCopper, Layer::BottomCopper);
    assert_eq!(reports(through, Layer::Inner(0)), 1);
    assert_eq!(reports(through, Layer::Inner(1)), 1);
}

#[test]
fn a_buried_via_is_not_on_either_face() {
    let buried = (Layer::Inner(0), Layer::Inner(1));
    assert_eq!(reports(buried, Layer::TopCopper), 0);
    assert_eq!(reports(buried, Layer::BottomCopper), 0);
    assert_eq!(reports(buried, Layer::Inner(0)), 1);
}
