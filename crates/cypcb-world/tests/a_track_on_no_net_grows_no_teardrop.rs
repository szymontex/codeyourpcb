//! A track on no net grows no teardrop.
//!
//! `cargo test -p cypcb-world --test a_track_on_no_net_grows_no_teardrop`
//!
//! A teardrop is copper of its track's net: it is spawned with that net's
//! `NetId` and the clearance rule treats it as the track. A track on no net
//! has no net to give it, and a fillet that took the first net's would be
//! copper the file never put on that net.

use cypcb_core::{Nm, Point, Rect};
use cypcb_world::components::trace::{Trace, TraceSegment};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::teardrop::{teardrops_of, TeardropRatios};
use cypcb_world::{BoardWorld, NetId, Position, RefDes, Rotation, Value};

fn board(track_net: Option<NetId>) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(
        Footprint {
            placement_origin: None,
            name: "PAD1".into(),
            description: String::new(),
            bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
            courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
            silk: Vec::new(),
            pads: vec![PadDef {
                hole_offset: None,
                number: "1".into(),
                shape: PadShape::Circle,
                position: Point::ORIGIN,
                size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
                drill: None,
                slot: None,
                layers: vec![Layer::TopCopper],
                mask_margin: None,
                rotation: Rotation::ZERO,
            }],
        }
        .with_courtyard_holding_its_pads(),
    );
    world.set_footprints(library.clone());
    let sig = world.intern_net("SIG");
    let mut connections = NetConnections::new();
    connections.add(PinConnection::new("1".to_string(), sig));
    world.spawn_component(
        RefDes::new("J1"),
        Value::new(""),
        Position(Point::from_mm(10.0, 15.0)),
        Rotation::ZERO,
        FootprintRef::new("PAD1"),
        connections,
    );
    let mut track = Trace::new(sig);
    track.net_id = track_net;
    track.width = Nm::from_mm(0.2);
    track.add_segment(TraceSegment::new(
        Point::from_mm(10.0, 15.0),
        Point::from_mm(20.0, 15.0),
    ));
    world.spawn_entity((track,));
    (world, library)
}

#[test]
fn a_track_on_a_net_grows_one_and_a_track_on_none_grows_none() {
    let (mut world, library) = board(Some(NetId::new(0)));
    let grown = teardrops_of(&mut world, &library, TeardropRatios::default());
    assert_eq!(grown.len(), 1, "the control");
    assert_eq!(grown[0].1, NetId::new(0));

    let (mut world, library) = board(None);
    assert!(teardrops_of(&mut world, &library, TeardropRatios::default()).is_empty());
}
