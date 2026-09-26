//! A pad is measured by its copper, and every rule asks the same question.
//!
//! `cargo test -p cypcb-drc --test a_pad_is_measured_by_its_shape`
//!
//! A rounded rectangle and an oblong were measured as the box around them. A
//! trace passing the box's corner clears the copper by the arc's cut, and read
//! as touching it. `UnroutedPinRule` asked whether copper reaches a pad with
//! boxes of its own: a via as its square, a trace as the box around each
//! segment, and a pad as its unturned size, so a pin was counted as reached by
//! copper that does not meet it. The mounting-hole, slot and edge rules
//! measured pads as their boxes too.
//!
//! Every pad is now a core box grown by a radius, the same `Copper` that
//! `ClearanceRule` and `NetSplitRule` measure, and every case below sits where
//! the box and the copper disagree. Every distance is worked from the centre.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{
    ClearanceRule, DrcRule, EdgeClearanceRule, MountingHoleClearanceRule, SlotClearanceRule,
    UnroutedPinRule,
};
use cypcb_drc::{shorts, ViolationKind};
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::components::{
    BoardOutline, FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position,
    RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A 0.1mm trace: half-width 0.05mm.
const HALF_WIDTH_NM: i64 = 50_000;
const CENTRE: i64 = 10_000_000;

fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
    world
}

fn pad(shape: PadShape, width_mm: f64, height_mm: f64) -> PadDef {
    PadDef {
        number: "1".to_string(),
        shape,
        position: Point::ORIGIN,
        size: (Nm::from_mm(width_mm), Nm::from_mm(height_mm)),
        drill: None,
        slot: None,
        layers: vec![Layer::TopCopper],
        mask_margin: None,
        rotation: Rotation::ZERO,
    }
}

/// Parts of one pad each, placed and turned, on a net or on none. The
/// footprints go into the world once every part is placed.
struct Parts {
    library: FootprintLibrary,
}

impl Parts {
    fn new() -> Self {
        Parts {
            library: FootprintLibrary::new(),
        }
    }

    fn place(
        &mut self,
        world: &mut BoardWorld,
        refdes: &str,
        pad: PadDef,
        at: (i64, i64),
        degrees: f64,
        net: Option<NetId>,
    ) {
        self.library.register(Footprint {
            name: refdes.to_string(),
            description: "one pad".to_string(),
            pads: vec![pad],
            bounds: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0))),
            courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0))),
            silk: Vec::new(),
        });
        let mut nets = NetConnections::new();
        if let Some(net) = net {
            nets.add(PinConnection::new("1".to_string(), net));
        }
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("pad"),
            Position(Point::new(Nm(at.0), Nm(at.1))),
            Rotation::from_degrees(degrees),
            FootprintRef::new(refdes),
            nets,
        );
    }

    fn done(self, world: &mut BoardWorld) {
        world.set_footprints(self.library);
        world.rebuild_spatial_index_with_traces(|_| {
            Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0)))
        });
    }
}

fn via(world: &mut BoardWorld, x_nm: i64, y_nm: i64, net: NetId) {
    world.spawn_entity((Via::new(Point::new(Nm(x_nm), Nm(y_nm)), net), net));
}

fn trace(world: &mut BoardWorld, from: (i64, i64), to: (i64, i64), net: NetId) {
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                Point::new(Nm(from.0), Nm(from.1)),
                Point::new(Nm(to.0), Nm(to.1)),
            )],
            width: Nm(2 * HALF_WIDTH_NM),
            layer: Layer::TopCopper,
            net_id: net,
            locked: false,
            source: TraceSource::Autorouted,
        },
        net,
    ));
}

/// A trace on the line `x + y = cx + cy + offset`, square to the diagonal
/// through `(cx, cy)`: its centreline is `offset / sqrt(2)` from that point.
fn diagonal_trace(world: &mut BoardWorld, at: (i64, i64), centreline_nm: f64, net: NetId) {
    let offset = (centreline_nm * std::f64::consts::SQRT_2).round() as i64;
    let sum = at.0 + at.1 + offset;
    let (x1, x2) = (at.0 - 2_000_000, at.0 + 2_000_000);
    trace(world, (x1, sum - x1), (x2, sum - x2), net);
}

/// A point `distance_nm` from `at` along the diagonal.
fn along_diagonal(at: (i64, i64), distance_nm: f64) -> (i64, i64) {
    let step = (distance_nm / std::f64::consts::SQRT_2).round() as i64;
    (at.0 + step, at.1 + step)
}

fn actual_mm(violations: &[cypcb_drc::DrcViolation]) -> Vec<f64> {
    violations
        .iter()
        .map(|v| v.actual.map(|n| n.0 as f64 / 1e6).unwrap_or(f64::NAN))
        .collect()
}

fn clearance(world: &mut BoardWorld) -> Vec<cypcb_drc::DrcViolation> {
    let rules = DesignRules {
        min_clearance: Nm::from_mm(0.127),
        ..DesignRules::jlcpcb_2layer()
    };
    ClearanceRule.check(world, &rules)
}

/// One gap under 0.127mm, not a short, and `expected_nm` wide.
fn assert_one_gap(found: &[cypcb_drc::DrcViolation], expected_nm: i64) {
    assert_eq!(
        found.len(),
        1,
        "one gap under 0.127mm: {:?}",
        actual_mm(found)
    );
    assert_eq!(found[0].kind, ViolationKind::Clearance);
    assert_eq!(shorts(found), 0, "board between the copper is not a short");
    let gap = found[0].actual.unwrap().0;
    assert!(
        (expected_nm - 1_500..=expected_nm + 1_500).contains(&gap),
        "the gap is {}mm, measured {}mm",
        expected_nm as f64 / 1e6,
        gap as f64 / 1e6
    );
}

#[test]
fn a_trace_past_the_corner_of_a_rounded_pads_box_is_a_gap_and_not_a_short() {
    // 1.0 x 1.0mm with 25% corners: radius 0.25mm, core corner 0.354mm out on
    // the diagonal, copper 0.604mm out, box corner 0.707mm out. A centreline
    // 0.70mm out runs inside the box and 0.096mm from the copper, so the gap
    // is 0.096 - 0.050 = 0.046mm.
    let mut world = board();
    let mut parts = Parts::new();
    parts.place(
        &mut world,
        "R1",
        pad(PadShape::RoundRect { corner_ratio: 25 }, 1.0, 1.0),
        (CENTRE, CENTRE),
        0.0,
        Some(NetId::new(1)),
    );
    parts.done(&mut world);
    diagonal_trace(&mut world, (CENTRE, CENTRE), 700_000.0, NetId::new(2));
    world.rebuild_spatial_index_with_traces(|_| {
        Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0)))
    });

    assert_one_gap(&clearance(&mut world), 46_447);
}

#[test]
fn a_trace_past_the_corner_of_an_oblongs_box_is_a_gap_and_not_a_short() {
    // 2.0 x 1.0mm: the segment from x = 9.5 to x = 10.5 grown by 0.5mm. From
    // the end of the segment the box corner is 0.707mm out on the diagonal and
    // the copper 0.5mm. A centreline 0.62mm out: gap 0.62 - 0.5 - 0.05.
    let mut world = board();
    let mut parts = Parts::new();
    parts.place(
        &mut world,
        "O1",
        pad(PadShape::Oblong, 2.0, 1.0),
        (CENTRE, CENTRE),
        0.0,
        Some(NetId::new(1)),
    );
    parts.done(&mut world);
    diagonal_trace(
        &mut world,
        (CENTRE + 500_000, CENTRE),
        620_000.0,
        NetId::new(2),
    );
    world.rebuild_spatial_index_with_traces(|_| {
        Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0)))
    });

    assert_one_gap(&clearance(&mut world), 70_000);
}

#[test]
fn a_trace_across_a_rounded_pads_corner_is_still_a_short() {
    // The control: a centreline 0.60mm out overlaps the copper, 0.604mm out.
    let mut world = board();
    let mut parts = Parts::new();
    parts.place(
        &mut world,
        "R1",
        pad(PadShape::RoundRect { corner_ratio: 25 }, 1.0, 1.0),
        (CENTRE, CENTRE),
        0.0,
        Some(NetId::new(1)),
    );
    parts.done(&mut world);
    diagonal_trace(&mut world, (CENTRE, CENTRE), 600_000.0, NetId::new(2));
    world.rebuild_spatial_index_with_traces(|_| {
        Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0)))
    });

    let found = clearance(&mut world);
    assert_eq!(
        shorts(&found),
        1,
        "a trace over a rounded pad's copper is a short: {:?}",
        actual_mm(&found)
    );
}

/// Net 1 with two pins: `TP1` at the centre, the pad under test, and `J1` at
/// (15.5, 10) with a trace from it heading for `TP1`. What reaches `TP1` is
/// what `reach` lays down.
fn unrouted_after(
    tp1: PadDef,
    degrees: f64,
    reach: impl FnOnce(&mut BoardWorld),
) -> Vec<cypcb_drc::DrcViolation> {
    let net = NetId::new(1);
    let mut world = board();
    let mut parts = Parts::new();
    parts.place(&mut world, "TP1", tp1, (CENTRE, CENTRE), degrees, Some(net));
    parts.place(
        &mut world,
        "J1",
        pad(PadShape::Rect, 0.6, 0.6),
        (15_500_000, CENTRE),
        0.0,
        Some(net),
    );
    parts.done(&mut world);
    trace(&mut world, (15_500_000, CENTRE), (13_000_000, CENTRE), net);
    reach(&mut world);
    UnroutedPinRule.check(&mut world, &DesignRules::jlcpcb_2layer())
}

fn unrouted_pins(found: &[cypcb_drc::DrcViolation]) -> Vec<String> {
    found
        .iter()
        .filter(|v| v.kind == ViolationKind::UnroutedPin)
        .map(|v| v.message.clone())
        .collect()
}

/// A via `distance_nm` from the pad's centre on the diagonal, joined to `J1`'s
/// trace.
fn via_on_the_diagonal(distance_nm: f64) -> impl FnOnce(&mut BoardWorld) {
    move |world: &mut BoardWorld| {
        let at = along_diagonal((CENTRE, CENTRE), distance_nm);
        via(world, at.0, at.1, NetId::new(1));
        trace(world, at, (13_000_000, CENTRE), NetId::new(1));
    }
}

#[test]
fn a_via_whose_square_reaches_a_pin_and_whose_disc_does_not_leaves_it_unrouted() {
    // A 0.6mm round pad and a 0.6mm via, centres 0.66mm apart on the
    // diagonal: 0.06mm of board between the discs, while their squares
    // overlap.
    let found = unrouted_after(
        pad(PadShape::Circle, 0.6, 0.6),
        0.0,
        via_on_the_diagonal(660_000.0),
    );

    assert_eq!(
        unrouted_pins(&found).len(),
        1,
        "copper that does not meet the pin leaves it unrouted: {:?}",
        unrouted_pins(&found)
    );
}

#[test]
fn a_via_whose_disc_meets_a_pin_reaches_it() {
    // The control: centres 0.55mm apart, the discs overlap by 0.05mm.
    let found = unrouted_after(
        pad(PadShape::Circle, 0.6, 0.6),
        0.0,
        via_on_the_diagonal(550_000.0),
    );

    assert!(
        unrouted_pins(&found).is_empty(),
        "copper that meets the pin reaches it: {:?}",
        unrouted_pins(&found)
    );
}

#[test]
fn a_trace_whose_box_covers_a_pin_and_whose_copper_misses_it_leaves_it_unrouted() {
    // A diagonal trace 0.383mm from a 0.6mm round pad's centre: the box around
    // the segment covers the pad, the copper stays 0.033mm clear of it.
    let found = unrouted_after(pad(PadShape::Circle, 0.6, 0.6), 0.0, |world| {
        diagonal_trace(world, (CENTRE, CENTRE), 383_000.0, NetId::new(1));
    });

    assert_eq!(
        unrouted_pins(&found).len(),
        1,
        "a trace that passes the pin does not reach it: {:?}",
        unrouted_pins(&found)
    );
}

#[test]
fn a_turned_pad_is_reached_where_its_copper_is() {
    // A 2.0 x 0.4mm pad turned 90 degrees spans x = 9.8 to 10.2. A trace
    // ending at x = 10.8 is 0.55mm short of it; unturned, the pad would span
    // x = 9.0 to 11.0 and cover the end.
    let short = unrouted_after(pad(PadShape::Rect, 2.0, 0.4), 90.0, |world| {
        trace(
            world,
            (13_000_000, CENTRE),
            (10_800_000, CENTRE),
            NetId::new(1),
        );
    });
    assert_eq!(
        unrouted_pins(&short).len(),
        1,
        "a trace that stops short of the turned pad does not reach it: {:?}",
        unrouted_pins(&short)
    );

    let onto = unrouted_after(pad(PadShape::Rect, 2.0, 0.4), 90.0, |world| {
        trace(
            world,
            (13_000_000, CENTRE),
            (10_200_000, CENTRE),
            NetId::new(1),
        );
    });
    assert!(
        unrouted_pins(&onto).is_empty(),
        "a trace ending on the turned pad's edge reaches it: {:?}",
        unrouted_pins(&onto)
    );
}

/// A 0.6mm round pad `distance_nm` from `from` on the diagonal, on a part of
/// its own, and whatever `hole` places at `from`.
fn round_pad_beside(
    from: (i64, i64),
    distance_nm: f64,
    hole: impl FnOnce(&mut BoardWorld, &mut Parts),
) -> BoardWorld {
    let mut world = board();
    let mut parts = Parts::new();
    hole(&mut world, &mut parts);
    parts.place(
        &mut world,
        "TP1",
        pad(PadShape::Circle, 0.6, 0.6),
        along_diagonal(from, distance_nm),
        0.0,
        Some(NetId::new(2)),
    );
    parts.done(&mut world);
    world
}

fn edge_rules() -> DesignRules {
    DesignRules {
        min_edge_clearance: Nm::from_mm(0.3),
        min_slot_clearance: Nm::from_mm(0.3),
        ..DesignRules::jlcpcb_2layer()
    }
}

fn mounting_hole(world: &mut BoardWorld, parts: &mut Parts) {
    let mut hole = pad(PadShape::Circle, 1.0, 1.0);
    hole.drill = Some(Nm::from_mm(1.0));
    hole.layers = Vec::new();
    parts.place(world, "H1", hole, (CENTRE, CENTRE), 0.0, None);
}

#[test]
fn a_round_pad_diagonal_to_a_mounting_hole_is_measured_by_its_disc() {
    // A 1.0mm unplated hole and a 0.6mm round pad, centres 1.150mm apart on the
    // diagonal: 1.15 - 0.5 - 0.3 = 0.35mm of board, which clears 0.3mm. The
    // pad's box corner is 0.226mm from the wall.
    let mut clear = round_pad_beside((CENTRE, CENTRE), 1_150_000.0, mounting_hole);
    let found = MountingHoleClearanceRule.check(&mut clear, &edge_rules());
    assert!(
        found.is_empty(),
        "0.35mm from the wall clears 0.3mm: {:?}",
        actual_mm(&found)
    );

    // The control: 0.950mm apart, 0.15mm of board.
    let mut close = round_pad_beside((CENTRE, CENTRE), 950_000.0, mounting_hole);
    let found = MountingHoleClearanceRule.check(&mut close, &edge_rules());
    assert_eq!(
        found.len(),
        1,
        "0.15mm from the wall: {:?}",
        actual_mm(&found)
    );
}

/// A plated 2.4 x 1.0mm slot along x at the centre: the bit's centre runs
/// from x = 9.3 to x = 10.7 and its radius is 0.5mm.
fn slot(world: &mut BoardWorld, parts: &mut Parts) {
    let mut slot = pad(PadShape::Oblong, 3.0, 1.6);
    slot.drill = Some(Nm::from_mm(1.0));
    slot.slot = Some((Nm::from_mm(2.4), Nm::from_mm(1.0)));
    slot.layers = vec![Layer::TopCopper, Layer::BottomCopper];
    parts.place(world, "S1", slot, (CENTRE, CENTRE), 0.0, None);
}

#[test]
fn a_round_pad_diagonal_to_a_slots_end_is_measured_by_its_disc() {
    // From the end of the slot's travel, as the hole above: 0.35mm of board.
    let end = (CENTRE + 700_000, CENTRE);
    let mut clear = round_pad_beside(end, 1_150_000.0, slot);
    let found = SlotClearanceRule.check(&mut clear, &edge_rules());
    assert!(
        found.is_empty(),
        "0.35mm from the slot clears 0.3mm: {:?}",
        actual_mm(&found)
    );

    let mut close = round_pad_beside(end, 950_000.0, slot);
    let found = SlotClearanceRule.check(&mut close, &edge_rules());
    assert_eq!(
        found.len(),
        1,
        "0.15mm from the slot: {:?}",
        actual_mm(&found)
    );
}

/// An L: 40 x 40mm with the top-right quarter removed, so (20, 20) is a
/// corner pointing into the board.
fn l_board(world: &mut BoardWorld) {
    let board = world.board_entity().unwrap();
    let outline = BoardOutline::new(vec![
        Point::from_mm(0.0, 0.0),
        Point::from_mm(40.0, 0.0),
        Point::from_mm(40.0, 20.0),
        Point::from_mm(20.0, 20.0),
        Point::from_mm(20.0, 40.0),
        Point::from_mm(0.0, 40.0),
    ])
    .expect("a ring");
    world.ecs_mut().entity_mut(board).insert(outline);
}

#[test]
fn a_round_pad_diagonal_to_an_inside_corner_is_measured_by_its_disc() {
    // A 0.6mm round pad whose centre is 0.707mm from the corner on the
    // diagonal: 0.407mm of board, which clears 0.35mm. Its box corner is
    // 0.283mm from the edge.
    let rules = DesignRules {
        min_edge_clearance: Nm::from_mm(0.35),
        ..DesignRules::jlcpcb_2layer()
    };
    let corner = (20_000_000, 20_000_000);

    let mut clear = round_pad_beside(corner, -707_107.0, |_, _| {});
    l_board(&mut clear);
    let found = EdgeClearanceRule.check(&mut clear, &rules);
    assert!(
        found.is_empty(),
        "0.407mm from the edge clears 0.35mm: {:?}",
        actual_mm(&found)
    );

    // The control: 0.424mm from the corner, 0.124mm of board.
    let mut close = round_pad_beside(corner, -424_264.0, |_, _| {});
    l_board(&mut close);
    let found = EdgeClearanceRule.check(&mut close, &rules);
    assert_eq!(
        found.len(),
        1,
        "0.124mm from the edge: {:?}",
        actual_mm(&found)
    );
}
