//! A through-hole pad is copper on every copper layer of the board.
//!
//! `cargo test -p cypcb-drc --test a_through_hole_pad_is_copper_on_every_layer`
//!
//! KiCad writes a plated through-hole pad `(layers "*.Cu" "*.Mask")`, and its
//! file format documents `*.Cu` as all of the copper layers. The importer can
//! only spell that as `TopCopper` and `BottomCopper`, because it reads a pad
//! before it knows how many layers the board has. Until 2026-09-25 every
//! consumer read that list: the router ended a trace on an inner layer at a
//! pin header - its goal takes any layer for a pad on more than one - and
//! the checks, which saw no inner copper there, reported the pin as reached
//! by nothing. Three of the four open pins of the `multi_ic` benchmark were
//! that. Another net's track run through the pad on an inner layer was, the
//! same way, copper on copper that no check saw.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{ClearanceRule, DrcRule, UnroutedPinRule};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position, RefDes,
    Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

const PAD_NET: NetId = NetId::new(1);
const OTHER_NET: NetId = NetId::new(2);

/// One 1.7mm round pad at the origin: drilled 1.0mm through the board, or a
/// top-side SMD pad of the same size, as the importer writes each.
fn pad(through_hole: bool) -> PadDef {
    let size = Nm::from_mm(1.7);
    PadDef {
        number: "1".to_string(),
        shape: PadShape::Circle,
        position: Point::ORIGIN,
        size: (size, size),
        drill: through_hole.then(|| Nm::from_mm(1.0)),
        slot: None,
        layers: if through_hole {
            vec![
                Layer::TopCopper,
                Layer::BottomCopper,
                Layer::TopMask,
                Layer::BottomMask,
            ]
        } else {
            vec![Layer::TopCopper, Layer::TopMask]
        },
        mask_margin: None,
    }
}

/// A four-layer board with the pad of net 1 at 10mm, 10mm, a through-hole pad
/// of the same net at 2mm, 10mm for a track to start from - a net of one pad
/// is never reported open - and one track.
fn board(through_hole: bool, track: (NetId, Layer, Point, Point)) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 4);

    let mut library = FootprintLibrary::new();
    let size = Nm::from_mm(1.7);
    for (name, through_hole) in [("PAD", through_hole), ("START", true)] {
        library.register(Footprint {
            name: name.to_string(),
            description: String::new(),
            pads: vec![pad(through_hole)],
            bounds: Rect::from_center_size(Point::ORIGIN, (size, size)),
            courtyard: Rect::from_center_size(Point::ORIGIN, (size, size)),
            silk: Vec::new(),
        });
    }
    for (refdes, x, footprint) in [("J1", 10.0, "PAD"), ("J2", 2.0, "START")] {
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", PAD_NET));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("pin"),
            Position::from_mm(x, 10.0),
            Rotation(0),
            FootprintRef::new(footprint),
            nets,
        );
    }

    let (net, layer, start, end) = track;
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(start, end)],
            width: Nm::from_mm(0.2),
            layer,
            net_id: net,
            locked: false,
            source: TraceSource::Autorouted,
        },
        net,
    ));

    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);
    world
}

/// A track of the pad's own net on `layer` that ends at `end`.
fn own_track(layer: Layer, end: Point) -> (NetId, Layer, Point, Point) {
    (PAD_NET, layer, Point::from_mm(2.0, 10.0), end)
}

/// A track of another net on `layer`, straight across the pad and clear of J2.
fn crossing(layer: Layer) -> (NetId, Layer, Point, Point) {
    (
        OTHER_NET,
        layer,
        Point::from_mm(5.0, 10.0),
        Point::from_mm(18.0, 10.0),
    )
}

/// Open pins of J1, the pad under test.
fn open_pins(world: &mut BoardWorld) -> usize {
    UnroutedPinRule
        .check(world, &DesignRules::default())
        .iter()
        .filter(|violation| violation.message.starts_with("J1."))
        .count()
}

fn shorts(world: &mut BoardWorld) -> usize {
    cypcb_drc::shorts(&ClearanceRule.check(world, &DesignRules::default()))
}

#[test]
fn a_track_that_ends_at_the_pad_on_an_inner_layer_reaches_it() {
    let centre = Point::from_mm(10.0, 10.0);
    for layer in [Layer::Inner(0), Layer::Inner(1)] {
        assert_eq!(
            open_pins(&mut board(true, own_track(layer, centre))),
            0,
            "a plated hole is copper on {layer:?}"
        );
    }
    // The control: the same track stopped 5mm short leaves the pin open, so
    // the rule is looking at this track and not passing everything.
    assert_eq!(
        open_pins(&mut board(
            true,
            own_track(Layer::Inner(0), Point::from_mm(5.0, 10.0))
        )),
        1
    );
    // A pad with no hole is on the layers it lists and no others.
    assert_eq!(
        open_pins(&mut board(false, own_track(Layer::Inner(0), centre))),
        1
    );
}

#[test]
fn another_net_across_the_pad_on_an_inner_layer_is_a_short() {
    for layer in [Layer::Inner(0), Layer::Inner(1)] {
        assert_eq!(
            shorts(&mut board(true, crossing(layer))),
            1,
            "another net through a plated hole on {layer:?}"
        );
    }
    // The faces, reported before and after: the control that the board is
    // built the way the inner cases assume.
    assert_eq!(shorts(&mut board(true, crossing(Layer::TopCopper))), 1);
    assert_eq!(shorts(&mut board(true, crossing(Layer::BottomCopper))), 1);
    // A top-side SMD pad is not on an inner layer, nor on the bottom.
    assert_eq!(shorts(&mut board(false, crossing(Layer::Inner(0)))), 0);
    assert_eq!(shorts(&mut board(false, crossing(Layer::BottomCopper))), 0);
    assert_eq!(shorts(&mut board(false, crossing(Layer::TopCopper))), 1);
}

#[test]
fn the_mask_names_every_copper_layer_for_a_plated_hole_and_none_for_a_bare_one() {
    let every_inner = [Layer::Inner(0), Layer::Inner(1), Layer::Inner(29)];
    let tht = pad(true);
    for layer in every_inner {
        assert!(tht.is_on(layer), "{layer:?}");
    }
    assert!(tht.is_on(Layer::TopCopper) && tht.is_on(Layer::BottomCopper));
    // Only the copper layers grow: the mask and paste lists stay as written.
    assert!(tht.is_on(Layer::TopMask) && !tht.is_on(Layer::TopPaste));

    let smd = pad(false);
    assert_eq!(smd.copper_mask(), Layer::TopCopper.to_copper_mask());

    let mut bare = pad(true);
    bare.layers.clear();
    assert!(bare.is_non_plated());
    assert_eq!(bare.copper_mask(), 0);
}
