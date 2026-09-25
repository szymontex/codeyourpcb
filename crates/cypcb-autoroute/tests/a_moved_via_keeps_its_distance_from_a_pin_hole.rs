//! A via the optimizer moves keeps the hole-to-hole distance from every hole
//! on the board, a through-hole pin's included.
//!
//! The optimizer knew a pad only as copper, and copper of its own net as
//! nothing at all. A hop over a fine-pitch pad went to one via past the pad's
//! end, 0.40mm from the hole of a connector pin beside it, and the checker
//! then reported the hole-to-hole fault the optimizer had made. The pin's hole
//! is now read from its pad, where the checker reads it.

use cypcb_autoroute::via_optimizer::{optimize_vias, BoardObstacles};
use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
use cypcb_router::types::{RouteSegment, ViaPlacement};
use cypcb_world::components::trace::Via;
use cypcb_world::components::{FootprintRef, Layer, NetConnections, PadShape, PinConnection};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, Position, RefDes, Rotation, Value};

/// What stands beside the fine-pitch pad the route hops over.
enum Beside {
    Nothing,
    /// A through-hole pin: its net is the route's when `own` is set, its
    /// centre, its pad size, its drill, its slot in the pad's frame and the
    /// part's rotation.
    Pin {
        own: bool,
        at: (f64, f64),
        size: (f64, f64),
        drill: f64,
        slot: Option<(f64, f64)>,
        degrees: f64,
    },
    /// A via of the route's net the designer placed.
    Via {
        at: (f64, f64),
    },
}

fn footprint(
    name: &str,
    size: (f64, f64),
    drill: Option<f64>,
    slot: Option<(f64, f64)>,
) -> Footprint {
    Footprint {
        name: name.into(),
        description: String::new(),
        bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
        courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".into(),
            shape: if drill.is_some() {
                PadShape::Circle
            } else {
                PadShape::Rect
            },
            position: Point::ORIGIN,
            size: (Nm::from_mm(size.0), Nm::from_mm(size.1)),
            drill: drill.map(Nm::from_mm),
            slot: slot.map(|(w, h)| (Nm::from_mm(w), Nm::from_mm(h))),
            layers: if drill.is_some() {
                vec![Layer::TopCopper, Layer::BottomCopper]
            } else {
                vec![Layer::TopCopper]
            },
            mask_margin: None,
        }],
    }
}

/// A route on Bottom that hops to Top across the fine-pitch pad U1 at
/// (15, 10), whose neighbours U2 and U3 the pair of vias rings, so the
/// optimizer moves the pair to one via past an end of U1. Returns the vias
/// it kept and the hole-to-hole faults the checker finds on the result.
fn optimize(beside: Beside, neighbours: bool) -> (Vec<ViaPlacement>, usize) {
    optimize_hop(beside, neighbours, 0.2)
}

/// The same, with the vias `half_hop` mm above and below U1's centre line.
fn optimize_hop(beside: Beside, neighbours: bool, half_hop: f64) -> (Vec<ViaPlacement>, usize) {
    let rules = DesignRules::jlcpcb_2layer();
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(footprint("FINE", (1.2, 0.3), None, None));
    if let Beside::Pin {
        size, drill, slot, ..
    } = &beside
    {
        library.register(footprint("PIN", *size, Some(*drill), *slot));
    }
    world.set_footprints(library.clone());

    let sig = world.intern_net("SIG");
    let mut parts = vec![("U1", (15.0, 10.0), "FINE", sig, 0.0)];
    if neighbours {
        parts.push(("U2", (15.0, 10.5), "FINE", world.intern_net("A"), 0.0));
        parts.push(("U3", (15.0, 9.5), "FINE", world.intern_net("B"), 0.0));
    }
    match beside {
        Beside::Nothing => {}
        Beside::Pin {
            own, at, degrees, ..
        } => {
            let net = if own { sig } else { world.intern_net("OTHER") };
            parts.push(("J1", at, "PIN", net, degrees));
        }
        Beside::Via { at } => {
            let via = Via::new(Point::from_mm(at.0, at.1), sig);
            world.ecs_mut().spawn((via, sig));
        }
    }
    for (refdes, (x, y), name, net, degrees) in parts {
        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), net));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new(""),
            Position(Point::from_mm(x, y)),
            Rotation::from_degrees(degrees),
            FootprintRef::new(name),
            connections,
        );
    }
    world.rebuild_spatial_index_from_library(&library);

    let segment = |layer, from: (f64, f64), to: (f64, f64)| {
        RouteSegment::new(
            sig,
            layer,
            Nm::from_mm(0.15),
            Point::from_mm(from.0, from.1),
            Point::from_mm(to.0, to.1),
        )
    };
    let segments = vec![
        segment(Layer::BottomCopper, (10.0, 11.0), (15.2, 10.0 + half_hop)),
        segment(
            Layer::TopCopper,
            (15.2, 10.0 + half_hop),
            (15.2, 10.0 - half_hop),
        ),
        segment(Layer::BottomCopper, (15.2, 10.0 - half_hop), (15.2, 5.0)),
    ];
    let via = |y, start, end| {
        ViaPlacement::new(sig, Point::from_mm(15.2, y), Nm::from_mm(0.2), start, end)
    };
    let vias = vec![
        via(10.0 + half_hop, Layer::BottomCopper, Layer::TopCopper),
        via(10.0 - half_hop, Layer::TopCopper, Layer::BottomCopper),
    ];

    let board = BoardObstacles::from_board(&mut world, &library);
    let (_, kept) = optimize_vias(
        segments,
        vias,
        &board,
        Nm::from_mm(0.1),
        rules.min_hole_to_hole,
    );

    for placed in &kept {
        let mut via = Via::new(placed.position, placed.net_id);
        via.drill = placed.drill;
        via.outer_diameter = placed.outer_diameter;
        via.start_layer = placed.start_layer;
        via.end_layer = placed.end_layer;
        world.ecs_mut().spawn((via, placed.net_id));
    }
    let faults = run_drc(&mut world, &rules)
        .violations
        .iter()
        .filter(|violation| violation.kind == ViolationKind::HoleToHole)
        .count();
    (kept, faults)
}

/// Laminate between a via's hole and a hole whose bit travels from `from` to
/// `to` with radius `radius`, in mm, measured here rather than by the checker
/// so a test does not share the code it tests.
fn laminate(via: &ViaPlacement, from: (f64, f64), to: (f64, f64), radius: f64) -> f64 {
    let p = (via.position.x.0 as f64 / 1e6, via.position.y.0 as f64 / 1e6);
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length2 = dx * dx + dy * dy;
    let t = if length2 == 0.0 {
        0.0
    } else {
        (((p.0 - from.0) * dx + (p.1 - from.1) * dy) / length2).clamp(0.0, 1.0)
    };
    let (qx, qy) = (from.0 + t * dx, from.1 + t * dy);
    ((p.0 - qx).powi(2) + (p.1 - qy).powi(2)).sqrt() - radius - via.drill.0 as f64 / 2e6
}

const HOLE_TO_HOLE_MM: f64 = 0.5;

#[test]
fn a_hop_with_nothing_beside_it_moves_past_the_nearer_end_of_the_pad() {
    // The control: the site the pins below are set against. Without it a pin
    // that drove the via to the other end would prove nothing.
    let (kept, faults) = optimize(Beside::Nothing, true);
    let at: Vec<_> = kept
        .iter()
        .map(|v| (v.position.x.0, v.position.y.0))
        .collect();
    assert_eq!(at, vec![(15_900_000, 10_000_000)]);
    assert_eq!(faults, 0);
}

#[test]
fn a_moved_via_keeps_its_distance_from_the_hole_of_a_pin_of_its_own_net() {
    let (kept, faults) = optimize(
        Beside::Pin {
            own: true,
            at: (16.8, 10.0),
            size: (1.0, 1.0),
            drill: 0.8,
            slot: None,
            degrees: 0.0,
        },
        true,
    );
    assert_eq!(kept.len(), 1, "the pair still becomes one via: {kept:?}");
    let gap = laminate(&kept[0], (16.8, 10.0), (16.8, 10.0), 0.4);
    assert!(
        gap >= HOLE_TO_HOLE_MM,
        "{gap:.3}mm to the pin's hole: {kept:?}"
    );
    assert_eq!(faults, 0);
}

#[test]
fn a_moved_via_keeps_its_distance_from_the_hole_of_a_pin_of_another_net() {
    // Its copper is 0.4mm off, which clears 0.1mm plus the ring; its hole is
    // 0.4mm off too, which does not clear 0.5mm.
    let (kept, faults) = optimize(
        Beside::Pin {
            own: false,
            at: (16.8, 10.0),
            size: (1.0, 1.0),
            drill: 0.8,
            slot: None,
            degrees: 0.0,
        },
        true,
    );
    assert_eq!(kept.len(), 1, "the pair still becomes one via: {kept:?}");
    let gap = laminate(&kept[0], (16.8, 10.0), (16.8, 10.0), 0.4);
    assert!(
        gap >= HOLE_TO_HOLE_MM,
        "{gap:.3}mm to the pin's hole: {kept:?}"
    );
    assert_eq!(faults, 0);
}

#[test]
fn a_moved_via_keeps_its_distance_from_the_near_end_of_a_turned_slot() {
    // A slot 2.4mm long, upright in its own frame and turned a quarter onto
    // the board's x axis: its centre is 1.5mm from the site past the pad and
    // clear of it, its near end 0.6mm off and not. Turned either way, so each
    // end of the travel is the near one once.
    for degrees in [90.0, 270.0] {
        let (kept, faults) = optimize(
            Beside::Pin {
                own: true,
                at: (17.4, 10.0),
                size: (1.0, 3.0),
                drill: 0.6,
                slot: Some((0.6, 2.4)),
                degrees,
            },
            true,
        );
        assert_eq!(kept.len(), 1, "{degrees}: one via: {kept:?}");
        let gap = laminate(&kept[0], (16.5, 10.0), (18.3, 10.0), 0.3);
        assert!(
            gap >= HOLE_TO_HOLE_MM,
            "{degrees}: {gap:.3}mm to the slot: {kept:?}"
        );
        assert_eq!(faults, 0, "{degrees}");
    }
}

#[test]
fn a_moved_via_keeps_its_distance_from_a_via_the_designer_placed() {
    let (kept, faults) = optimize(Beside::Via { at: (16.55, 10.0) }, true);
    assert_eq!(kept.len(), 1, "the pair still becomes one via: {kept:?}");
    let gap = laminate(&kept[0], (16.55, 10.0), (16.55, 10.0), 0.15);
    assert!(
        gap >= HOLE_TO_HOLE_MM,
        "{gap:.3}mm to the placed via: {kept:?}"
    );
    assert_eq!(faults, 0);
}

#[test]
fn a_wide_hop_that_fouls_nothing_stays() {
    // The control for the test below: vias 0.8mm apart, 0.6mm of laminate
    // between their holes, no neighbours and no pin. Nothing fouls, so the
    // pair stays where the router put it.
    let (kept, faults) = optimize_hop(Beside::Nothing, false, 0.4);
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert_eq!(faults, 0);
}

#[test]
fn a_pair_too_close_to_a_pin_hole_is_moved_off_it() {
    // The same pair with a pin of its net below it: the lower via's hole is
    // 0.2mm from the pin's, and nothing else fouls the pair.
    let (kept, faults) = optimize_hop(
        Beside::Pin {
            own: true,
            at: (15.2, 8.9),
            size: (1.0, 1.0),
            drill: 0.8,
            slot: None,
            degrees: 0.0,
        },
        false,
        0.4,
    );
    assert_eq!(kept.len(), 1, "the pair is traded for one via: {kept:?}");
    let gap = laminate(&kept[0], (15.2, 8.9), (15.2, 8.9), 0.4);
    assert!(
        gap >= HOLE_TO_HOLE_MM,
        "{gap:.3}mm to the pin's hole: {kept:?}"
    );
    assert_eq!(faults, 0);
}
