//! Copper that meets itself at an acute angle.
//!
//! Etching is a timed process: the board sits in the etchant until the copper
//! meant to go is gone, and comes out. Inside a sharp corner the etchant is
//! trapped against two faces at once and has nowhere to drain, so it keeps
//! working there after the rest of the board is finished - the corner is
//! undercut, the copper necks, and the trace fails somewhere it was drawn
//! full width. The usual name is an acid trap.
//!
//! **Ninety degrees is the line, and it is geometry rather than a fab
//! figure.** No house in the preset tables publishes an angle: what they
//! publish is a minimum copper feature size, which is a length and answers a
//! different question. A right angle drains; anything sharper does not. The
//! rule reports strictly below ninety, so a corner drawn square passes - and
//! the test for that is the one that dies first if the comparison is ever
//! written `<=`.
//!
//! The angle measured here is the one *between the two arms* of a junction: a
//! straight run is 180 degrees and a 45 degree turn leaves 135. That is not
//! the convention a trace entering a pad is measured in - there the angle is
//! between the edge of the trace and the edge of the land, where perpendicular
//! is 90 - so the same wedge of copper has two different numbers in the two
//! rules. Both are right; they measure different things.
//!
//! # What it will not do
//!
//! **Measure a T.** A junction here is two segment ends meeting at a point. A
//! segment end that lands in the *middle* of another segment is a junction
//! too, and this rule does not compute its angle - it reports it as not
//! checked, once per net, the way `ImpedanceRule` reports a cause once instead
//! of once per segment. Counted before the rule was written: over every routed
//! board in this repository, 184 junctions, of which **0** were T-joints.
//!
//! **Measure a curve.** A curved trace carries a `Curve` component and is
//! stored as the chords of the arc it draws, so every one of its interior
//! junctions is an angle nobody drew. Those traces are skipped whole.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::entity::Entity;
use cypcb_core::Point;
use cypcb_world::components::trace::{Curve, Trace};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for copper that meets itself at less than a right angle.
pub struct AcuteAngleRule;

/// One length of copper: the trace it belongs to, and the two ends it joins.
type Segment = (Entity, Point, Point);

/// Every segment of one net on one layer, which is where copper meets copper.
type Groups = BTreeMap<(u32, u32), Vec<Segment>>;

/// One arm of a junction: the segment's far end, and the trace it belongs to.
#[derive(Clone, Copy)]
struct Arm {
    far: Point,
    entity: Entity,
}

/// Is the angle between two arms sharper than a right angle?
///
/// Exactly, in integers. The angle at `p` is acute when the two directions
/// leaving it point the same way at all, which is the sign of their dot
/// product and nothing else - so a corner drawn square gives zero, is not
/// acute, and cannot be turned into one by a rounding error in a cosine.
fn is_acute(p: Point, a: Point, b: Point) -> bool {
    let (ax, ay) = ((a.x.0 - p.x.0) as i128, (a.y.0 - p.y.0) as i128);
    let (bx, by) = ((b.x.0 - p.x.0) as i128, (b.y.0 - p.y.0) as i128);
    ax * bx + ay * by > 0
}

/// Do two arms leave the same point along the same line?
///
/// Then the copper is not bent, it is drawn over itself - two paths laid down
/// the same corridor, or a trace that turns back along the way it came. There
/// is no wedge between them and no etchant to trap, so it is reported in
/// different words from a sharp corner.
fn is_collinear(p: Point, a: Point, b: Point) -> bool {
    let (ax, ay) = ((a.x.0 - p.x.0) as i128, (a.y.0 - p.y.0) as i128);
    let (bx, by) = ((b.x.0 - p.x.0) as i128, (b.y.0 - p.y.0) as i128);
    ax * by - ay * bx == 0
}

/// The angle between two arms in degrees, for the message only.
fn degrees(p: Point, a: Point, b: Point) -> f64 {
    let first = ((a.y.0 - p.y.0) as f64).atan2((a.x.0 - p.x.0) as f64);
    let second = ((b.y.0 - p.y.0) as f64).atan2((b.x.0 - p.x.0) as f64);
    let between = (first - second).to_degrees().abs() % 360.0;
    if between > 180.0 {
        360.0 - between
    } else {
        between
    }
}

/// Does `p` lie strictly inside the segment `a` to `b`?
///
/// Strictly: a point equal to either end is the corner case this rule does
/// measure, and only the interior is the one it does not.
fn inside(p: Point, a: Point, b: Point) -> bool {
    if (p.x.0, p.y.0) == (a.x.0, a.y.0) || (p.x.0, p.y.0) == (b.x.0, b.y.0) {
        return false;
    }
    let (dx, dy) = ((b.x.0 - a.x.0) as i128, (b.y.0 - a.y.0) as i128);
    let (px, py) = ((p.x.0 - a.x.0) as i128, (p.y.0 - a.y.0) as i128);
    if dx * py - dy * px != 0 {
        return false;
    }
    let along = px * dx + py * dy;
    along > 0 && along < dx * dx + dy * dy
}

impl DrcRule for AcuteAngleRule {
    fn name(&self) -> &'static str {
        "acute-angle"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let traces: Vec<(Entity, Trace)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(Entity, &Trace, Option<&Curve>)>();
            query
                .iter(ecs)
                .filter(|(_, _, curve)| curve.is_none())
                .map(|(entity, trace, _)| (entity, trace.clone()))
                .collect()
        };
        if traces.is_empty() {
            return Vec::new();
        }

        // Copper meets copper on one layer of one net. A via corner is two
        // junctions on two layers rather than one bend, and two nets crossing
        // on the same layer is a short - `ClearanceRule`'s question, not this
        // one.
        let mut groups: Groups = BTreeMap::new();
        for (entity, trace) in &traces {
            for segment in &trace.segments {
                // A segment with no length has no direction, so it has no
                // angle to either side of it.
                if (segment.start.x.0, segment.start.y.0) == (segment.end.x.0, segment.end.y.0) {
                    continue;
                }
                groups
                    .entry((trace.net_id.0, trace.layer.to_copper_mask()))
                    .or_default()
                    .push((*entity, segment.start, segment.end));
            }
        }

        let mut violations = Vec::new();
        let mut said: BTreeSet<u32> = BTreeSet::new();

        for ((net_id, _), segments) in &groups {
            let net_name = world
                .net_name(cypcb_world::NetId(*net_id))
                .unwrap_or("unnamed")
                .to_string();

            let mut ends: BTreeMap<(i64, i64), Vec<Arm>> = BTreeMap::new();
            for (entity, start, end) in segments {
                ends.entry((start.x.0, start.y.0)).or_default().push(Arm {
                    far: *end,
                    entity: *entity,
                });
                ends.entry((end.x.0, end.y.0)).or_default().push(Arm {
                    far: *start,
                    entity: *entity,
                });
            }

            for ((x, y), arms) in &ends {
                let at = Point::new(cypcb_core::Nm(*x), cypcb_core::Nm(*y));

                // A free end has one arm and no angle; a fanout has three or
                // more, and every pair of them is copper meeting copper.
                let sharpest = arms
                    .iter()
                    .enumerate()
                    .flat_map(|(i, first)| arms[i + 1..].iter().map(move |second| (first, second)))
                    .filter(|(first, second)| is_acute(at, first.far, second.far))
                    .map(|(first, second)| {
                        (
                            degrees(at, first.far, second.far),
                            first.entity,
                            is_collinear(at, first.far, second.far),
                        )
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));

                // One report per junction, not one per pair: a designer moves
                // the corner, and the corner is the place.
                if let Some((angle, entity, overlapping)) = sharpest {
                    let message = if overlapping {
                        format!(
                            "net '{net_name}': copper leaves this point twice along the same line, \
                             so it is drawn over itself here rather than bent - \
                             no wedge, and no copper anybody meant either"
                        )
                    } else {
                        format!(
                            "net '{net_name}': copper meets itself at {angle:.1} degrees, \
                             and an angle under 90 traps etchant against both faces - \
                             the corner keeps etching after the board is done"
                        )
                    };
                    violations.push(DrcViolation::acid_trap(entity, message, at));
                }
            }

            // The junctions this rule cannot measure, said once for the net.
            let tee = ends.keys().find(|(x, y)| {
                let p = Point::new(cypcb_core::Nm(*x), cypcb_core::Nm(*y));
                segments.iter().any(|(_, a, b)| inside(p, *a, *b))
            });
            if let Some((x, y)) = tee {
                if said.insert(*net_id) {
                    let at = Point::new(cypcb_core::Nm(*x), cypcb_core::Nm(*y));
                    let entity = segments[0].0;
                    let mut violation = DrcViolation::acid_trap(entity, String::new(), at);
                    violation.message = format!(
                        "net '{net_name}': copper ends in the middle of its own trace here, \
                         and the angle of that junction is not computed. \
                         Not checked - not passed"
                    );
                    violations.push(violation);
                }
            }
        }

        violations
    }
}
