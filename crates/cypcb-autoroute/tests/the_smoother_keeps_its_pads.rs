//! Does a pad its net's copper touched still have that copper on it after the
//! smoother has moved it?
//!
//! `cargo test -p cypcb-autoroute --test the_smoother_keeps_its_pads`
//!
//! A path does not always end on the pad it reaches. On `multi_ic` the
//! VCC_3V3 trunk came along y = 45mm and turned down at x = 30.6mm, a corner
//! that lies on pad 2 of R8, the 0402 at 30mm, 45mm. Nothing ended there, so
//! the smoother chamfered the corner, 0.514mm off the pad, and the pin was
//! left open. The shape is rebuilt here from the router's copper, and the pad
//! is read by `UnroutedPinRule`, the check a fabricated board answers.

use cypcb_autoroute::smoother::{smooth_routes, smooth_routes_on_pads};
use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{net_pads, DrcRule, UnroutedPinRule};
use cypcb_router::types::RouteSegment;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position, RefDes,
    Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// The router's default `roundness`.
const ROUNDNESS: f64 = 0.5;

const VCC: NetId = NetId::new(1);
const OTHER: NetId = NetId::new(2);

/// A two-pad part with pads of `width` by `height` at -0.48mm and 0.48mm, as
/// `R_0402_1005Metric` places them.
fn part(width: f64, height: f64) -> Footprint {
    let pad = |number: &str, x: f64| PadDef {
        number: number.to_string(),
        shape: PadShape::RoundRect { corner_ratio: 25 },
        position: Point::from_mm(x, 0.0),
        size: (Nm::from_mm(width), Nm::from_mm(height)),
        drill: None,
        slot: None,
        layers: vec![Layer::TopCopper, Layer::TopMask],
        mask_margin: None,
        rotation: Rotation::ZERO,
    };
    let size = (Nm::from_mm(1.0 + width), Nm::from_mm(height));
    Footprint {
        name: "PART".to_string(),
        description: String::new(),
        pads: vec![pad("1", -0.48), pad("2", 0.48)],
        bounds: Rect::from_center_size(Point::ORIGIN, size),
        courtyard: Rect::from_center_size(Point::ORIGIN, size),
        silk: Vec::new(),
    }
}

/// R8 at 30mm, 45mm with pad 2 on VCC, and R9 whose pad 1 the trunk starts
/// from at 37.8mm, 45mm - a net of one pad is never reported open.
fn board(pad: (f64, f64)) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(60.0), Nm::from_mm(60.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(part(pad.0, pad.1));
    for (refdes, x, pins) in [
        ("R8", 30.0, [("1", OTHER), ("2", VCC)]),
        ("R9", 38.28, [("1", VCC), ("2", OTHER)]),
    ] {
        let mut nets = NetConnections::new();
        for (pin, net) in pins {
            nets.add(PinConnection::new(pin, net));
        }
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("r"),
            Position::from_mm(x, 45.0),
            Rotation(0),
            FootprintRef::new("PART"),
            nets,
        );
    }
    world.set_footprints(library.clone());
    (world, library)
}

/// The VCC trunk as the router laid it: along y = 45mm from R9 and down at
/// x = 30.6mm, on R8 pad 2.
fn trunk() -> Vec<RouteSegment> {
    let segment = |from: (f64, f64), to: (f64, f64)| {
        RouteSegment::new(
            VCC,
            Layer::TopCopper,
            Nm::from_mm(0.1),
            Point::from_mm(from.0, from.1),
            Point::from_mm(to.0, to.1),
        )
    };
    vec![
        segment((37.8, 45.0), (30.6, 45.0)),
        segment((30.6, 45.0), (30.6, 48.2)),
    ]
}

/// Whether R8 pad 2 is open with `copper` laid on the board.
fn r8_open(world: &mut BoardWorld, library: &FootprintLibrary, copper: &[RouteSegment]) -> bool {
    for seg in copper {
        world.spawn_entity((
            Trace {
                segments: vec![TraceSegment::new(seg.start, seg.end)],
                width: seg.width,
                layer: seg.layer,
                net_id: seg.net_id,
                locked: false,
                source: TraceSource::Autorouted,
            },
            seg.net_id,
        ));
    }
    world.rebuild_spatial_index_from_library(library);
    UnroutedPinRule
        .check(world, &DesignRules::default())
        .iter()
        .any(|violation| violation.message.starts_with("R8.2"))
}

/// The trunk smoothed with the board's pads, and without them.
fn smoothed(pad: (f64, f64)) -> (Vec<RouteSegment>, Vec<RouteSegment>) {
    let (mut world, library) = board(pad);
    let pads = net_pads(&mut world, &library);
    (
        smooth_routes_on_pads(&trunk(), &[], &[], &pads, Nm(0), ROUNDNESS),
        smooth_routes(&trunk(), &[], &[], Nm(0), ROUNDNESS),
    )
}

#[test]
fn a_corner_on_a_pad_is_not_cut_off_it() {
    let pad = (0.56, 0.62);
    let (kept, blind) = smoothed(pad);

    let (mut world, library) = board(pad);
    assert!(
        !r8_open(&mut world, &library, &trunk()),
        "the router's own copper has to reach R8.2, or the test measures nothing"
    );
    // The control: smoothed without the pads, the chamfer takes the corner
    // off R8.2, which is what the router shipped until 2026-09-25.
    let (mut world, library) = board(pad);
    assert!(
        r8_open(&mut world, &library, &blind),
        "without the pads the smoother no longer cuts R8.2 off, so this test \
         no longer rebuilds the fault:\n{blind:#?}"
    );
    let (mut world, library) = board(pad);
    assert!(
        !r8_open(&mut world, &library, &kept),
        "the smoother moved the VCC trunk off R8.2:\n{kept:#?}"
    );
}

#[test]
fn a_corner_whose_chamfer_stays_on_the_pad_is_still_cut() {
    // A pad 2mm square: the 0.514mm chamfer lands on it too, and the
    // smoother is held only where the pad would lose its copper.
    let pad = (2.0, 2.0);
    let (kept, blind) = smoothed(pad);
    assert_eq!(kept, blind, "a chamfer that keeps the pad was given up");
    assert_ne!(kept, trunk(), "the corner was not chamfered at all");
    let (mut world, library) = board(pad);
    assert!(!r8_open(&mut world, &library, &kept));
}
