//! The wedge where a trace enters a land, measured along the trace's edges.
//!
//! R-08 asks how sharply copper meets a land. R-03 asks a related question and
//! is not this one: it measures the angle between the two arms of a junction,
//! so a straight run is one hundred and eighty degrees, while here the copper
//! meets the land's boundary and a straight run into the middle of a side is
//! ninety. The same wedge carries two numbers in the two rules because the two
//! rules measure different things.
//!
//! **The measurement runs along the trace's edges, not along its axis.** On a
//! round land a radial entry puts the axis through the centre, where every
//! tangent is square to it, so an axis measurement reads ninety for every
//! radial entry and a rule built on it would sit in the registry passing every
//! board forever. The edges are offset by half the trace's width and leave the
//! land where the copper actually narrows.
//!
//! A straight line crosses a convex outline at most twice, and all four pad
//! shapes in this model are convex, so "which crossing" has one answer: the
//! one with the larger parameter, where the edge leaves the land. The smaller
//! one is where it enters and is not a wedge.
//!
//! Two edges give two angles and the answer is the **smaller**. An entry near
//! a corner leaves by two different sides, and taking either one rather than
//! the minimum reports the blunter half of a wedge that is already too sharp.
//!
//! A refusal is a value with a name rather than a silence. A trace wider than
//! the land it enters reports one instead of an angle: width against a land is
//! `NeckDownRule`'s question, and an angle invented here would be a number
//! about nothing.
//!
//! The inputs are integers in nanometres. The only inexact steps are the
//! crossing parameter and the arc tangent, which is where floating point
//! enters; the answer leaves as an integer in millidegrees, the same unit this
//! project already uses for rotations and arc sweeps, so an assertion on it
//! has no epsilon to choose.

use bevy_ecs::entity::Entity;
use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Trace;
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation};
use cypcb_world::footprint::PadDef;
use cypcb_world::BoardWorld;
use cypcb_world::{Pad, PadShape};

use super::{layer_bit, rotate_point, DrcRule};
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// Below this angle the entry is a violation. Strict: exactly this value is
/// clean, the way R-03 treats exactly ninety.
pub const ENTRY_ANGLE_MIN_MDEG: u32 = 45_000;

/// Why an entry carries no angle.
///
/// Every one of these is a case where a number would be an invention rather
/// than a measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRefusal {
    /// The trace is wider than the land's smaller dimension. That is
    /// `NeckDownRule`'s question, not this one.
    TraceWiderThanLand,
    /// The land has no width or no height, so it has no boundary to meet.
    PadHasNoSize,
    /// The segment's two ends are the same point, so the copper arrives from
    /// no direction.
    ArmHasNoDirection,
    /// Neither edge of the trace crosses the land's boundary on its way out.
    /// A trace ending at a corner and heading straight away from the land does
    /// this, and there is no wedge to measure.
    EdgeMissesLand,
}

/// What an entry measured, or why it did not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// The angle between the trace's edge and the land's boundary where the
    /// edge leaves it, in millidegrees, in the range zero to ninety thousand.
    Measured {
        /// The angle in millidegrees.
        millideg: u32,
    },
    /// No angle, and the reason.
    NotChecked(EntryRefusal),
}

impl Entry {
    /// The angle if one was measured.
    pub fn millideg(self) -> Option<u32> {
        match self {
            Entry::Measured { millideg } => Some(millideg),
            Entry::NotChecked(_) => None,
        }
    }

    /// Whether this entry is sharper than the rule allows. A refusal is not a
    /// violation: it is an absence of a measurement.
    pub fn is_violation(self) -> bool {
        matches!(self, Entry::Measured { millideg } if millideg < ENTRY_ANGLE_MIN_MDEG)
    }
}

/// A land's outline in its own frame, centred on the origin and axis-aligned.
///
/// Placement - where the pad sits and how it is turned - is the caller's
/// problem, so that the geometry here can be tested without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadOutline {
    /// The pad's shape.
    pub shape: PadShape,
    /// Full width across the x axis.
    pub width: Nm,
    /// Full height across the y axis.
    pub height: Nm,
}

impl PadOutline {
    /// The outline of a pad, ignoring where it is placed.
    pub fn of(pad: &Pad) -> Self {
        PadOutline {
            shape: pad.shape,
            width: pad.width,
            height: pad.height,
        }
    }
}

/// Whether a point lies in the land's copper, in the pad's own frame.
///
/// The boundary counts as inside. A trace ending perfectly on the edge of a
/// land should not fall out of this rule and out of the exporter's teardrop at
/// once, for one reason nobody chose.
///
/// This shares the corner radius with `boundary_of` rather than restating it,
/// because a point the boundary walk thinks is outside and this thinks is
/// inside is a wedge measured against a land the copper never entered.
pub fn outline_contains(outline: PadOutline, p: Point) -> bool {
    if outline.width.raw() <= 0 || outline.height.raw() <= 0 {
        return false;
    }
    let w = outline.width.raw() as f64 / 2.0;
    let h = outline.height.raw() as f64 / 2.0;
    let x = (p.x.raw() as f64).abs();
    let y = (p.y.raw() as f64).abs();
    if let PadShape::Circle = outline.shape {
        return x * x + y * y <= w * w + EPS_NM;
    }
    if x > w + EPS_NM || y > h + EPS_NM {
        return false;
    }
    let r = corner_radius(outline, w, h);
    if r <= 0.0 {
        return true;
    }
    // Outside the corner's quarter only when the point is past both inner
    // edges; anywhere else the straight sides already answered.
    let (dx, dy) = (x - (w - r), y - (h - r));
    if dx <= 0.0 || dy <= 0.0 {
        return true;
    }
    dx * dx + dy * dy <= r * r + EPS_NM
}

/// The corner radius each shape rounds its rectangle by, in nanometres.
fn corner_radius(outline: PadOutline, w: f64, h: f64) -> f64 {
    let radius = match outline.shape {
        PadShape::Circle | PadShape::Rect => 0.0,
        PadShape::RoundRect { corner_ratio } => w.min(h) * 2.0 * f64::from(corner_ratio) / 100.0,
        PadShape::Oblong => w.min(h),
    };
    radius.min(w).min(h).max(0.0)
}

/// One piece of a land's boundary.
///
/// A tangent to a circle is the radius turned ninety degrees, so the arc case
/// covers a round land, a rounded rectangle's corners and an oblong's ends
/// with one expression.
enum Boundary {
    Segment {
        ax: f64,
        ay: f64,
        bx: f64,
        by: f64,
    },
    Arc {
        cx: f64,
        cy: f64,
        r: f64,
        /// Start of the swept range, in radians, counter-clockwise.
        from: f64,
        /// End of the swept range, in radians, counter-clockwise from `from`.
        to: f64,
    },
}

/// Distances below this, in nanometres, are the same point. A nanometre is the
/// smallest distance this model carries, so anything under half of one is
/// arithmetic noise rather than geometry.
const EPS_NM: f64 = 0.5;

/// Two crossings whose parameters differ by less than this are the same
/// crossing seen from two boundary pieces - a corner. Both tangents are real
/// there and the smaller angle wins.
const EPS_CORNER_NM: f64 = 1.0;

/// The angle where a trace entering a land leaves it, in the land's own frame.
///
/// `end` is the segment end that lies in the land, `arm` is the segment's
/// other end, and `width` is the trace's width. The trace runs from `end`
/// towards `arm`, so that is the direction its edges leave by.
pub fn entry_angle(outline: PadOutline, end: Point, arm: Point, width: Nm) -> Entry {
    if outline.width.raw() <= 0 || outline.height.raw() <= 0 {
        return Entry::NotChecked(EntryRefusal::PadHasNoSize);
    }
    if width.raw() > outline.width.raw().min(outline.height.raw()) {
        return Entry::NotChecked(EntryRefusal::TraceWiderThanLand);
    }

    let dx = (arm.x.raw() - end.x.raw()) as f64;
    let dy = (arm.y.raw() - end.y.raw()) as f64;
    let len = (dx * dx + dy * dy).sqrt();
    if len < EPS_NM {
        return Entry::NotChecked(EntryRefusal::ArmHasNoDirection);
    }
    let (dx, dy) = (dx / len, dy / len);

    let boundary = boundary_of(outline);
    let half = width.raw() as f64 / 2.0;
    // The perpendicular to the direction of travel. One edge sits on each
    // side; a trace with no width would put both on the axis, which is the
    // measurement this rule exists to avoid.
    let (px, py) = (-dy, dx);
    let ex = end.x.raw() as f64;
    let ey = end.y.raw() as f64;

    let edges = [
        leaving_angle(&boundary, ex + px * half, ey + py * half, dx, dy),
        leaving_angle(&boundary, ex - px * half, ey - py * half, dx, dy),
    ];

    match edges.iter().flatten().copied().reduce(f64::min) {
        Some(degrees) => Entry::Measured {
            millideg: (degrees * 1000.0).round() as u32,
        },
        None => Entry::NotChecked(EntryRefusal::EdgeMissesLand),
    }
}

/// The same measurement for a pad the board has placed.
///
/// The geometry above works in the pad's own frame and knows nothing about
/// placement. This is the change of frame and nothing else: the pad's centre
/// is where the footprint puts it once the component is turned, and the
/// trace's two points are carried into that frame rather than the outline
/// being carried out of it - a rectangle stays a rectangle that way, and only
/// two points pay for the rotation.
pub fn entry_angle_placed(
    pad: &PadDef,
    at: Point,
    rotation_deg: f64,
    end: Point,
    arm: Point,
    width: Nm,
) -> Entry {
    entry_angle(
        outline_of(pad),
        into_pad_frame(pad, at, rotation_deg, end),
        into_pad_frame(pad, at, rotation_deg, arm),
        width,
    )
}

/// Whether a board point lies in a placed pad's copper.
///
/// The board walk asks this of both ends of every segment, and it has to be
/// the same containment the boundary walk uses or the walk would hand
/// `entry_angle` a pair of points it disagrees with about which one is inside.
pub fn pad_contains(pad: &PadDef, at: Point, rotation_deg: f64, p: Point) -> bool {
    outline_contains(outline_of(pad), into_pad_frame(pad, at, rotation_deg, p))
}

/// The outline a footprint's pad definition describes, without its placement.
fn outline_of(pad: &PadDef) -> PadOutline {
    PadOutline {
        shape: pad.shape,
        width: pad.size.0,
        height: pad.size.1,
    }
}

/// A board point carried into the pad's own frame.
///
/// One transform, used by the angle and by the containment test, because two
/// copies of it are two chances for a point to be inside for one of them and
/// outside for the other.
/// Where a pad's centre lands once the part is placed and turned.
///
/// Public because a diagnostic that asks where the sharp entries sit needs
/// the centre the measurement itself used. Computing it a second time in the
/// caller is how two readings of the same pad start to disagree.
pub fn pad_centre(pad: &PadDef, at: Point, rotation_deg: f64) -> Point {
    let offset = rotate_point(pad.position, rotation_deg);
    Point::from_raw(at.x.raw() + offset.x.raw(), at.y.raw() + offset.y.raw())
}

fn into_pad_frame(pad: &PadDef, at: Point, rotation_deg: f64, p: Point) -> Point {
    let centre = pad_centre(pad, at, rotation_deg);
    rotate_point(
        Point::from_raw(p.x.raw() - centre.x.raw(), p.y.raw() - centre.y.raw()),
        -rotation_deg,
    )
}

/// The angle in degrees where one ray leaves the outline, or `None` if it
/// never meets it.
fn leaving_angle(boundary: &[Boundary], px: f64, py: f64, dx: f64, dy: f64) -> Option<f64> {
    let mut crossings: Vec<(f64, f64)> = Vec::new();
    for piece in boundary {
        piece.hits(px, py, dx, dy, &mut crossings);
    }
    let last = crossings
        .iter()
        .map(|&(t, _)| t)
        .fold(f64::NEG_INFINITY, f64::max);
    if !last.is_finite() {
        return None;
    }
    // At a corner two pieces answer at the same parameter and both tangents
    // are real. The smaller angle is the one that describes the wedge.
    crossings
        .iter()
        .filter(|&&(t, _)| t > last - EPS_CORNER_NM)
        .map(|&(_, angle)| angle)
        .reduce(f64::min)
}

impl Boundary {
    /// Every crossing this piece has with the ray, as parameter and angle in
    /// degrees between the ray and the boundary's tangent there.
    fn hits(&self, px: f64, py: f64, dx: f64, dy: f64, out: &mut Vec<(f64, f64)>) {
        match *self {
            Boundary::Segment { ax, ay, bx, by } => {
                let (ex, ey) = (bx - ax, by - ay);
                let denom = dx * ey - dy * ex;
                if denom.abs() < f64::EPSILON {
                    return;
                }
                let t = ((ax - px) * ey - (ay - py) * ex) / denom;
                let s = ((ax - px) * dy - (ay - py) * dx) / denom;
                // `s` runs from zero to one along the side whatever its
                // length, so the tolerance on it is a nanometre expressed in
                // that parameter rather than a nanometre.
                let len = (ex * ex + ey * ey).sqrt();
                let slack = EPS_NM / len;
                if t > EPS_NM && (-slack..=1.0 + slack).contains(&s) {
                    out.push((t, acute_between(dx, dy, ex / len, ey / len)));
                }
            }
            Boundary::Arc {
                cx,
                cy,
                r,
                from,
                to,
            } => {
                let (fx, fy) = (px - cx, py - cy);
                let b = fx * dx + fy * dy;
                let c = fx * fx + fy * fy - r * r;
                let disc = b * b - c;
                if disc < 0.0 {
                    return;
                }
                let root = disc.sqrt();
                for t in [-b - root, -b + root] {
                    if t <= EPS_NM {
                        continue;
                    }
                    let (hx, hy) = (fx + dx * t, fy + dy * t);
                    if !sweeps(from, to, hy.atan2(hx)) {
                        continue;
                    }
                    // A tangent to a circle is the radius turned ninety
                    // degrees.
                    out.push((t, acute_between(dx, dy, -hy / r, hx / r)));
                }
            }
        }
    }
}

/// Whether an angle lies in the range swept counter-clockwise from `from`.
fn sweeps(from: f64, to: f64, angle: f64) -> bool {
    let tau = std::f64::consts::TAU;
    // A full turn is the whole circle. Taking it modulo a turn first would
    // fold it back to nothing, which is how a round land came to have no
    // boundary at all.
    if to - from >= tau - 1e-9 {
        return true;
    }
    (angle - from).rem_euclid(tau) <= (to - from).rem_euclid(tau)
}

/// The acute angle in degrees between two unit vectors, so that a tangent
/// pointing either way along the same boundary gives the same answer.
fn acute_between(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    (ax * bx + ay * by)
        .abs()
        .clamp(0.0, 1.0)
        .acos()
        .to_degrees()
}

/// The pieces of a land's boundary, counter-clockwise.
fn boundary_of(outline: PadOutline) -> Vec<Boundary> {
    let w = outline.width.raw() as f64 / 2.0;
    let h = outline.height.raw() as f64 / 2.0;
    match outline.shape {
        PadShape::Circle => vec![Boundary::Arc {
            cx: 0.0,
            cy: 0.0,
            r: w,
            from: 0.0,
            to: std::f64::consts::TAU,
        }],
        // Every other shape is a rectangle with its corners rounded by some
        // radius, and `corner_radius` is the one place that says by how much.
        _ => rounded(w, h, corner_radius(outline, w, h)),
    }
}

/// A rectangle whose corners are arcs of the given radius. A radius of zero is
/// a plain rectangle and a radius equal to the smaller half-dimension is a
/// stadium, so one function covers three of the four shapes.
fn rounded(w: f64, h: f64, radius: f64) -> Vec<Boundary> {
    let r = radius.min(w).min(h).max(0.0);
    let (iw, ih) = (w - r, h - r);
    let side = |ax: f64, ay: f64, bx: f64, by: f64| Boundary::Segment { ax, ay, bx, by };
    let mut pieces = vec![
        side(w, -ih, w, ih),
        side(-iw, h, iw, h),
        side(-w, -ih, -w, ih),
        side(-iw, -h, iw, -h),
    ];
    if r <= 0.0 {
        return pieces;
    }
    let quarter = std::f64::consts::FRAC_PI_2;
    for (index, (cx, cy)) in [(iw, ih), (-iw, ih), (-iw, -ih), (iw, -ih)]
        .into_iter()
        .enumerate()
    {
        pieces.push(Boundary::Arc {
            cx,
            cy,
            r,
            from: quarter * index as f64,
            to: quarter * (index as f64 + 1.0),
        });
    }
    pieces
}

/// What a board walk looked at, so a clean board can be told from an
/// unexamined one.
///
/// `DrcRule::check` returns a bare vector. An empty vector says "nothing is
/// wrong" and has no way to say "nothing was measured", and those are the two
/// readings a rule full of refusals sits between. Every figure here comes from
/// the same pass that produced the violations, so the denominator cannot drift
/// from the numerator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EntryReport {
    /// Segment ends found in a land's copper with the other end outside it, on
    /// a layer the pad is on and the net the pad is on.
    pub examined: usize,
    /// Of those, the ones `entry_angle` would not measure. Each is a named
    /// refusal, not a silence.
    pub refused: usize,
    /// Of those, the ones below [`ENTRY_ANGLE_MIN_MDEG`].
    pub violations: usize,
}

/// Every entry on the board, measured, with the count of what was looked at.
///
/// A segment is an entry when one end lies in a pad's copper and the other
/// does not. Both ends inside is copper crossing a land without leaving it -
/// there is no boundary in that segment to make a wedge against. Both ends
/// outside is a segment that passes over the land, which is a clearance
/// question and belongs to a different rule.
///
/// The pad's net and the pad's layers both have to match. A trace of another
/// net crossing a land is a short, reported by `ClearanceRule`; measuring its
/// entry angle would answer a question nobody asked about a board that is
/// already wrong.
/// One segment crossing one land's boundary, and everything the walk saw
/// about it.
///
/// [`measure_entries`] is a fold over these. Two walks over the same board are
/// two chances to disagree about which segments were entries at all, and a
/// denominator is only worth having when nothing can quietly count a different
/// set - so a diagnostic that asks a further question about the entries asks
/// it of these records rather than by walking the board again.
#[derive(Debug, Clone)]
pub struct EntryRecord {
    /// The component carrying the land.
    pub entity: Entity,
    /// `RefDes.pad`, spelled as a violation spells it.
    pub pin: String,
    /// The end of the segment that lies in the land's copper.
    pub inside: Point,
    /// The end that does not.
    pub outside: Point,
    /// The width this segment runs at, which is not always the trace's.
    pub width: Nm,
    /// Which trace on the board this segment belongs to. One `Trace` carries a
    /// whole net's copper rather than one pad-to-pad route, so two entries
    /// sharing this index are two lands entered by the same net.
    pub trace_index: usize,
    /// Whether the end inside the land is also an end of the whole trace.
    ///
    /// The Gerber writer fillets only a track *end* that lands inside a pad -
    /// "a track crossing a pad on its way elsewhere is not an entry and gets
    /// nothing", in `crates/cypcb-export/src/gerber/copper.rs`. That is a
    /// narrower set than the crossings this rule measures, and R-08's two
    /// halves are about different junctions unless this says otherwise.
    pub inside_is_trace_end: bool,
    /// Where this segment sits in its trace.
    pub segment_index: usize,
    /// How many segments that trace has.
    pub segment_count: usize,
    /// What the measurement made of the crossing.
    pub entry: Entry,
}

/// Every segment on the board that crosses a land's boundary, measured.
///
/// The walk: each placed component, each pad of its footprint that has a net,
/// each trace on that net and on a layer the pad is on, each segment with one
/// end in the land's copper and one end out of it.
pub fn entry_records(world: &mut BoardWorld) -> Vec<EntryRecord> {
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };
    let components: Vec<_> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(
            Entity,
            &RefDes,
            &FootprintRef,
            &NetConnections,
            &Position,
            &Rotation,
        )>();
        query
            .iter(ecs)
            .map(|(e, r, f, n, p, rot)| (e, r.clone(), f.clone(), n.clone(), *p, *rot))
            .collect()
    };

    let library = world.footprints();
    let mut records = Vec::new();

    for (entity, refdes, footprint_ref, nets, position, rotation) in &components {
        let Some(footprint) = library.get(footprint_ref.as_str()) else {
            continue; // Unknown footprint - sync already reported it
        };
        let rotation_deg = rotation.to_degrees();

        for pad in &footprint.pads {
            let Some(net) = nets.pin_net(&pad.number) else {
                continue; // No net - `UnconnectedPinRule`'s question
            };
            // A pad whose layer list names no copper this code understands is
            // treated as being on every layer rather than on none, the way
            // `UnroutedPinRule` treats it: refusing to measure because the
            // footprint spells its layers unfamiliarly would be reporting the
            // reader rather than the board.
            let mask: u32 = pad
                .layers
                .iter()
                .filter_map(|layer| layer_bit(*layer))
                .fold(0, |mask, bit| mask | bit);
            let mask = if mask == 0 { u32::MAX } else { mask };

            for (trace_index, trace) in traces.iter().enumerate() {
                if trace.net_id != net {
                    continue;
                }
                match layer_bit(trace.layer) {
                    Some(bit) if mask & bit != 0 => {}
                    _ => continue,
                }

                for (segment_index, segment) in trace.segments.iter().enumerate() {
                    let start_in = pad_contains(pad, position.0, rotation_deg, segment.start);
                    let end_in = pad_contains(pad, position.0, rotation_deg, segment.end);
                    let (inside, outside) = match (start_in, end_in) {
                        (true, false) => (segment.start, segment.end),
                        (false, true) => (segment.end, segment.start),
                        _ => continue,
                    };

                    // The segment's own width, not the trace's, when it has
                    // one. The stretch that enters a pad is exactly the one a
                    // `neck` declaration makes thinner, so a walk that reads
                    // the trace's width would measure the wrong wedge on the
                    // commonest entry this rule exists for.
                    let width = segment.width.unwrap_or(trace.width);
                    // The polyline's own two ends, which are the only points
                    // the exporter's fillet can grow from.
                    let same =
                        |a: Point, b: Point| a.x.raw() == b.x.raw() && a.y.raw() == b.y.raw();
                    let inside_is_trace_end = (segment_index == 0 && same(inside, segment.start))
                        || (segment_index + 1 == trace.segments.len() && same(inside, segment.end));
                    records.push(EntryRecord {
                        entity: *entity,
                        pin: format!("{}.{}", refdes.as_str(), pad.number),
                        inside,
                        outside,
                        width,
                        inside_is_trace_end,
                        trace_index,
                        segment_index,
                        segment_count: trace.segments.len(),
                        entry: entry_angle_placed(
                            pad,
                            position.0,
                            rotation_deg,
                            inside,
                            outside,
                            width,
                        ),
                    });
                }
            }
        }
    }

    records
}

/// The violations R-08 reports, and the denominator they came out of.
pub fn measure_entries(world: &mut BoardWorld) -> (Vec<DrcViolation>, EntryReport) {
    let mut violations = Vec::new();
    let mut report = EntryReport::default();

    for record in entry_records(world) {
        report.examined += 1;
        // `Entry::is_violation` owns the comparison against the threshold. A
        // second `millideg < ENTRY_ANGLE_MIN_MDEG` here would be a two-place
        // decision, which is how a strict bound gets loosened in one place and
        // left in the other.
        match record.entry {
            Entry::NotChecked(_) => report.refused += 1,
            Entry::Measured { millideg } if record.entry.is_violation() => {
                report.violations += 1;
                violations.push(DrcViolation::pad_entry(
                    record.entity,
                    record.pin,
                    millideg,
                    ENTRY_ANGLE_MIN_MDEG,
                    record.inside,
                ));
            }
            Entry::Measured { .. } => {}
        }
    }

    (violations, report)
}

/// Rule that reports a trace meeting a land too sharply.
///
/// R-08. The threshold is [`ENTRY_ANGLE_MIN_MDEG`] and it is not a
/// `DesignRules` field, because no fabricator publishes it: it is what R-08's
/// own sources say, and a value read from a fab table would be a number
/// nobody stated.
///
/// The rule drops the denominator [`measure_entries`] returns, because
/// `DrcRule::check` has nowhere to put one. That is why the function is public
/// and tested on the report directly - the count exists and is checked, even
/// where the trait cannot carry it.
pub struct PadEntryRule;

impl DrcRule for PadEntryRule {
    fn name(&self) -> &'static str {
        "pad-entry"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        measure_entries(world).0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::components::Layer;

    fn round_land(diameter: i64) -> PadOutline {
        PadOutline {
            shape: PadShape::Circle,
            width: Nm::new(diameter),
            height: Nm::new(diameter),
        }
    }

    fn rect_land(width: i64, height: i64) -> PadOutline {
        PadOutline {
            shape: PadShape::Rect,
            width: Nm::new(width),
            height: Nm::new(height),
        }
    }

    /// The control that decides whether this rule can ever fire.
    ///
    /// A 1.2 mm trace entering a 1.6 mm round land along a radius: the edges
    /// sit 0.6 from the centre of a land of radius 0.8, so each leaves where
    /// the tangent stands at ninety less asin of three quarters. Measured
    /// along the *axis* instead, a radial entry reads ninety for every trace
    /// on every round land, and the rule would pass every board forever.
    #[test]
    fn a_radial_entry_into_a_round_land_is_measured_along_the_edges() {
        let entry = entry_angle(
            round_land(1_600_000),
            Point::from_raw(0, 0),
            Point::from_raw(0, 5_000_000),
            Nm::new(1_200_000),
        );
        assert_eq!(entry, Entry::Measured { millideg: 41_410 });
        assert!(entry.is_violation());
    }

    /// The same land with a narrower trace passes, so the test above is
    /// measuring the trace rather than the land.
    #[test]
    fn a_narrower_trace_into_the_same_land_is_clean() {
        let entry = entry_angle(
            round_land(1_600_000),
            Point::from_raw(0, 0),
            Point::from_raw(0, 5_000_000),
            Nm::new(800_000),
        );
        assert_eq!(entry, Entry::Measured { millideg: 60_000 });
        assert!(!entry.is_violation());
    }

    /// A trace arriving along the top side of a 2.0 by 1.0 land. One edge
    /// clears the land entirely and never meets it; the other runs parallel to
    /// the top side and leaves by the right one, square to it.
    ///
    /// This case was written down as "zero degrees" before anybody computed
    /// it. There is no wedge here - what is left beside the copper is a narrow
    /// sliver, which is a minimum-feature question and not an entry angle.
    #[test]
    fn a_trace_arriving_along_a_side_leaves_square_by_the_next_one() {
        let entry = entry_angle(
            rect_land(2_000_000, 1_000_000),
            Point::from_raw(0, 500_000),
            Point::from_raw(3_000_000, 500_000),
            Nm::new(250_000),
        );
        assert_eq!(entry, Entry::Measured { millideg: 90_000 });
        assert!(!entry.is_violation());
    }

    /// The second control that names a wrong implementation. The entry runs at
    /// thirty degrees near a corner, so one edge leaves by the top side at
    /// thirty and the other by the right side at sixty. An answer of sixty
    /// thousand means an edge was taken rather than the smaller of the two.
    #[test]
    fn the_answer_is_the_smaller_of_the_two_edges() {
        let entry = entry_angle(
            rect_land(2_000_000, 2_000_000),
            Point::from_raw(0, 422_650),
            Point::from_raw(866_025, 922_650),
            Nm::new(250_000),
        );
        assert_eq!(entry, Entry::Measured { millideg: 30_000 });
        assert!(entry.is_violation());
    }

    /// The threshold is strict, the way R-03 pins exactly ninety: an entry at
    /// exactly forty-five degrees is clean.
    #[test]
    fn exactly_the_threshold_is_clean() {
        let entry = entry_angle(
            rect_land(2_000_000, 2_000_000),
            Point::from_raw(0, 0),
            Point::from_raw(1_000_000, 1_000_000),
            Nm::new(250_000),
        );
        assert_eq!(
            entry,
            Entry::Measured {
                millideg: ENTRY_ANGLE_MIN_MDEG
            }
        );
        assert!(!entry.is_violation());
    }

    /// An oblong entered along its short axis meets a flat side square on, so
    /// the round-land arithmetic is not applied to a shape that has straight
    /// copper where the trace arrives.
    #[test]
    fn an_oblong_entered_across_its_flat_side_reads_square() {
        let entry = entry_angle(
            PadOutline {
                shape: PadShape::Oblong,
                width: Nm::new(3_200_000),
                height: Nm::new(1_600_000),
            },
            Point::from_raw(0, 0),
            Point::from_raw(0, 5_000_000),
            Nm::new(1_200_000),
        );
        assert_eq!(entry, Entry::Measured { millideg: 90_000 });
    }

    /// Width against a land is `NeckDownRule`'s question. An angle invented
    /// here would be a number about nothing.
    #[test]
    fn a_trace_wider_than_the_land_is_refused_rather_than_measured() {
        let entry = entry_angle(
            round_land(1_600_000),
            Point::from_raw(0, 0),
            Point::from_raw(0, 5_000_000),
            Nm::new(2_000_000),
        );
        assert_eq!(entry, Entry::NotChecked(EntryRefusal::TraceWiderThanLand));
        assert!(!entry.is_violation());
        assert_eq!(entry.millideg(), None);
    }

    /// A trace ending at a corner and heading straight away from the land: its
    /// two edges pass outside the boundary and never cross it. Without this
    /// case the code divides by a crossing that is not there.
    #[test]
    fn an_entry_whose_edges_both_miss_the_land_is_refused() {
        let entry = entry_angle(
            rect_land(2_000_000, 2_000_000),
            Point::from_raw(1_000_000, 1_000_000),
            Point::from_raw(3_000_000, 3_000_000),
            Nm::new(250_000),
        );
        assert_eq!(entry, Entry::NotChecked(EntryRefusal::EdgeMissesLand));
    }

    /// An edge that begins outside the land crosses the boundary twice, and
    /// only the second crossing is the wedge. Here the far edge enters by the
    /// left side at sixty-three degrees and leaves by the top at twenty-six,
    /// so an implementation taking the nearer crossing reports sixty-three
    /// thousand four hundred and thirty-five.
    #[test]
    fn the_crossing_that_matters_is_where_the_edge_leaves() {
        let entry = entry_angle(
            rect_land(2_000_000, 2_000_000),
            Point::from_raw(-900_000, 100_000),
            Point::from_raw(1_100_000, 1_100_000),
            Nm::new(1_000_000),
        );
        assert_eq!(entry, Entry::Measured { millideg: 26_565 });
        assert!(entry.is_violation());
    }

    fn square_pad(offset: Point) -> PadDef {
        PadDef {
            number: "1".to_string(),
            shape: PadShape::Rect,
            position: offset,
            size: (Nm::new(2_000_000), Nm::new(2_000_000)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        }
    }

    /// The change of frame, exercised where it can go wrong: a pad that sits
    /// away from its footprint's origin, on a component that has been turned.
    /// The entry is the thirty-degree one above, carried into board
    /// coordinates, and every rotation must read it back unchanged.
    ///
    /// Turning the trace's two points rather than the outline is what keeps
    /// this exact: a rectangle stays axis-aligned in its own frame, and two
    /// points rounded to the nanometre move the direction by less than a
    /// thousandth of a degree.
    #[test]
    fn a_rotated_pad_reads_the_same_angle() {
        let pad = square_pad(Point::from_raw(1_000_000, 0));
        let at = Point::from_raw(10_000_000, 4_000_000);
        for rotation in [0.0f64, 90.0, 180.0, 37.0, -45.0] {
            let offset = rotate_point(pad.position, rotation);
            let centre = Point::from_raw(at.x.raw() + offset.x.raw(), at.y.raw() + offset.y.raw());
            let place = |p: Point| {
                let turned = rotate_point(p, rotation);
                Point::from_raw(
                    centre.x.raw() + turned.x.raw(),
                    centre.y.raw() + turned.y.raw(),
                )
            };
            let entry = entry_angle_placed(
                &pad,
                at,
                rotation,
                place(Point::from_raw(0, 422_650)),
                place(Point::from_raw(866_025, 922_650)),
                Nm::new(250_000),
            );
            assert_eq!(
                entry,
                Entry::Measured { millideg: 30_000 },
                "rotation {rotation}"
            );
        }
    }

    /// The property the specification asked for, measured rather than
    /// asserted - and it is false.
    ///
    /// Sweeping the arm across every direction from an end off the land's
    /// centre line, the largest step between consecutive tenth-degree samples
    /// is 42 500 millidegrees. A continuous measurement would step by about
    /// the sample size. This is the same defect as the case below, sized: the
    /// answer does not drift at a corner, it changes side.
    #[test]
    fn the_answer_is_not_continuous_as_the_arm_sweeps() {
        let land = rect_land(2_000_000, 2_000_000);
        let end = Point::from_raw(0, 422_650);
        let mut previous: Option<u32> = None;
        let mut largest_step = 0u32;
        let mut refused = 0usize;
        for step in 0..=900 {
            let degrees = 0.5 + f64::from(step) * 0.1;
            let radians = degrees.to_radians();
            let arm = Point::from_raw(
                end.x.raw() + (5_000_000.0 * radians.cos()).round() as i64,
                end.y.raw() + (5_000_000.0 * radians.sin()).round() as i64,
            );
            match entry_angle(land, end, arm, Nm::new(250_000)).millideg() {
                Some(measured) => {
                    if let Some(before) = previous {
                        largest_step = largest_step.max(before.abs_diff(measured));
                    }
                    previous = Some(measured);
                }
                None => {
                    refused += 1;
                    previous = None;
                }
            }
        }
        assert_eq!(refused, 0, "every direction from inside the land measures");
        assert_eq!(largest_step, 42_500);
    }

    /// A known limit, pinned rather than left to be discovered.
    ///
    /// The measurement asks which side of the land the trace's edge crosses,
    /// and near a corner that question flips with a hair's movement. Entering
    /// the middle of a square land at 39.9 degrees, the upper edge still
    /// leaves by the right side and the answer is 50 100; a tenth of a degree
    /// later it leaves by the top instead and the answer is 40 000. Nothing
    /// about the copper changed by ten degrees.
    ///
    /// The wedge against the other side is real on both sides of that
    /// threshold - it is simply not the side the edge crosses. So this rule
    /// under-reports beside a corner, and the case that matters is the one
    /// just before the flip, where a trap is present and the number is clean.
    /// Fixing it means measuring against boundary pieces the edge does not
    /// cross, which needs a distance to be invented, and this project does not
    /// invent distances.
    #[test]
    fn the_answer_jumps_where_the_leaving_side_changes() {
        let land = rect_land(2_000_000, 2_000_000);
        let end = Point::from_raw(0, 0);
        let just_before = entry_angle(
            land,
            end,
            Point::from_raw(3_835_826, 3_207_248),
            Nm::new(250_000),
        );
        let just_after = entry_angle(
            land,
            end,
            Point::from_raw(3_830_222, 3_213_938),
            Nm::new(250_000),
        );
        assert_eq!(just_before, Entry::Measured { millideg: 50_100 });
        assert_eq!(just_after, Entry::Measured { millideg: 40_000 });
        assert!(!just_before.is_violation());
        assert!(just_after.is_violation());
    }

    /// The boundary counts as inside, on the straight side and on the corner
    /// arc alike.
    #[test]
    fn a_point_on_the_boundary_is_inside() {
        assert!(outline_contains(
            rect_land(2_000_000, 1_000_000),
            Point::from_raw(1_000_000, 0)
        ));
        assert!(outline_contains(
            round_land(1_600_000),
            Point::from_raw(800_000, 0)
        ));
        assert!(!outline_contains(
            round_land(1_600_000),
            Point::from_raw(800_001, 0)
        ));
    }

    /// The corner of a rounded rectangle is the case a bounding box gets
    /// wrong: the point is inside the rectangle and outside the copper.
    #[test]
    fn a_rounded_corner_is_not_its_bounding_box() {
        let land = PadOutline {
            shape: PadShape::RoundRect { corner_ratio: 25 },
            width: Nm::new(2_000_000),
            height: Nm::new(2_000_000),
        };
        // The radius is a quarter of the smaller dimension: 0.5 of 2.0.
        assert!(outline_contains(land, Point::from_raw(0, 0)));
        assert!(outline_contains(land, Point::from_raw(999_999, 0)));
        assert!(!outline_contains(
            land,
            Point::from_raw(1_000_000, 1_000_000)
        ));
        // Just inside the corner arc, on its diagonal.
        assert!(outline_contains(land, Point::from_raw(853_000, 853_000)));
        assert!(!outline_contains(land, Point::from_raw(860_000, 860_000)));
    }

    /// An oblong is a stadium, so past the flat part it curves away well
    /// before its bounding box does.
    #[test]
    fn an_oblong_ends_in_a_half_circle() {
        let land = PadOutline {
            shape: PadShape::Oblong,
            width: Nm::new(3_200_000),
            height: Nm::new(1_600_000),
        };
        assert!(outline_contains(land, Point::from_raw(1_600_000, 0)));
        assert!(outline_contains(land, Point::from_raw(800_000, 800_000)));
        assert!(!outline_contains(land, Point::from_raw(1_500_000, 700_000)));
    }

    #[test]
    fn a_land_with_no_size_contains_nothing() {
        assert!(!outline_contains(
            rect_land(0, 1_000_000),
            Point::from_raw(0, 0)
        ));
    }

    #[test]
    fn a_segment_with_no_length_has_no_direction() {
        let entry = entry_angle(
            round_land(1_600_000),
            Point::from_raw(0, 0),
            Point::from_raw(0, 0),
            Nm::new(250_000),
        );
        assert_eq!(entry, Entry::NotChecked(EntryRefusal::ArmHasNoDirection));
    }

    #[test]
    fn a_land_with_no_size_has_no_boundary_to_meet() {
        let entry = entry_angle(
            rect_land(0, 1_000_000),
            Point::from_raw(0, 0),
            Point::from_raw(1_000_000, 0),
            Nm::new(250_000),
        );
        assert_eq!(entry, Entry::NotChecked(EntryRefusal::PadHasNoSize));
    }

    // ---- the board walk ----------------------------------------------------
    //
    // The geometry above is tested in the pad's own frame. These test the walk
    // that finds the pairs of points to hand it, which is a different thing
    // and fails differently: it can look at the wrong copper, or at none.

    use cypcb_world::components::trace::TraceSegment;
    use cypcb_world::components::{PinConnection, Value};
    use cypcb_world::footprint::{Footprint, FootprintLibrary};
    use cypcb_world::NetId;

    /// A board with one part carrying one round land of `land_mm`, placed at
    /// `offset_mm` in the footprint and turned by `rotation`.
    fn board_with_land(
        land_mm: f64,
        offset_mm: (f64, f64),
        rotation: Rotation,
        layers: Vec<Layer>,
    ) -> (BoardWorld, NetId) {
        board_with_shaped_land(
            PadShape::Circle,
            (land_mm, land_mm),
            offset_mm,
            rotation,
            layers,
        )
    }

    /// The same, for a land that is not round and not square, so that turning
    /// the geometry round is not the identity.
    fn board_with_shaped_land(
        shape: PadShape,
        size_mm: (f64, f64),
        offset_mm: (f64, f64),
        rotation: Rotation,
        layers: Vec<Layer>,
    ) -> (BoardWorld, NetId) {
        let mut world = BoardWorld::new();
        world.set_board("entry".into(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
        let net = world.intern_net("N");

        let mut library = FootprintLibrary::new();
        let base = library
            .get("0402")
            .expect("the library has an 0402")
            .clone();
        library.register_design(Footprint {
            name: "pin".to_string(),
            pads: vec![PadDef {
                number: "1".to_string(),
                shape,
                position: Point::from_mm(offset_mm.0, offset_mm.1),
                size: (Nm::from_mm(size_mm.0), Nm::from_mm(size_mm.1)),
                drill: None,
                slot: None,
                layers,
                mask_margin: None,
            }],
            ..base
        });
        world.set_footprints(library);

        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", net));
        world.spawn_component(
            RefDes::new("J1"),
            Value::new(""),
            Position::from_mm(10.0, 10.0),
            rotation,
            FootprintRef::new("pin"),
            NetConnections::clone(&nets),
        );
        (world, net)
    }

    /// One straight run of `width_mm` from `from_mm` to `to_mm`.
    fn add_trace(
        world: &mut BoardWorld,
        net: NetId,
        layer: Layer,
        width_mm: f64,
        from_mm: (f64, f64),
        to_mm: (f64, f64),
    ) {
        let mut trace = Trace::new(net);
        trace.layer = layer;
        trace.width = Nm::from_mm(width_mm);
        trace.segments.push(TraceSegment::new(
            Point::from_mm(from_mm.0, from_mm.1),
            Point::from_mm(to_mm.0, to_mm.1),
        ));
        world.spawn_entity((trace,));
    }

    /// A trace of several segments, built one point at a time.
    fn add_polyline(
        world: &mut BoardWorld,
        net: NetId,
        layer: Layer,
        width_mm: f64,
        points: &[(f64, f64)],
    ) {
        let mut trace = Trace::new(net);
        trace.layer = layer;
        trace.width = Nm::from_mm(width_mm);
        for pair in points.windows(2) {
            trace.segments.push(TraceSegment::new(
                Point::from_mm(pair[0].0, pair[0].1),
                Point::from_mm(pair[1].0, pair[1].1),
            ));
        }
        world.spawn_entity((trace,));
    }

    #[test]
    fn a_record_says_where_in_its_trace_the_entering_segment_sits() {
        // Two traces into the same land, both entering on their own last
        // segment, one after a three segment run and one after none. Both
        // arrive radially and both therefore read the same angle, so the
        // position in the trace is the only thing that tells the records
        // apart - which is what a question about segment order needs.
        let (mut world, net) = board_with_land(
            1.6,
            (0.0, 0.0),
            Rotation::ZERO,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        add_polyline(
            &mut world,
            net,
            Layer::TopCopper,
            0.2,
            &[(4.0, 4.0), (6.0, 6.0), (8.0, 8.0), (10.0, 10.0)],
        );
        add_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.2,
            (10.0, 15.0),
            (10.0, 10.0),
        );

        let records = entry_records(&mut world);
        assert_eq!(records.len(), 2, "two traces cross the land's boundary");

        let mut seen: Vec<(usize, usize)> = records
            .iter()
            .map(|r| (r.segment_index, r.segment_count))
            .collect();
        seen.sort_unstable();
        assert_eq!(
            seen,
            vec![(0, 1), (2, 3)],
            "the three segment trace enters on its third segment and the one segment trace on its first"
        );

        // Both tracks end in the land, so both would be filleted - the
        // positive arm of the pass-through case below, without which a reader
        // that always answered "not an end" would pass it.
        assert!(
            records.iter().all(|r| r.inside_is_trace_end),
            "both tracks have their own end inside the land"
        );

        // The two earlier segments of the long trace are not entries: neither
        // end of them is in the land's copper, so the walk never reaches them.
        assert!(
            records
                .iter()
                .all(|r| r.segment_index + 1 == r.segment_count),
            "both entries are the last segment of their own trace"
        );
    }

    #[test]
    fn a_track_passing_through_a_land_ends_nowhere_in_it() {
        // Two crossings of one land by a track that carries on past it. The
        // rule measures both, and the Gerber writer fillets neither: it grows
        // a teardrop from a track's own end, and this track's ends are four
        // millimetres away on either side. R-08's two halves are not about the
        // same junctions, and this is the case that says so.
        let (mut world, net) = board_with_land(
            1.6,
            (0.0, 0.0),
            Rotation::ZERO,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        add_polyline(
            &mut world,
            net,
            Layer::TopCopper,
            0.2,
            &[(6.0, 6.0), (10.0, 10.0), (14.0, 6.0)],
        );

        let records = entry_records(&mut world);
        assert_eq!(records.len(), 2, "the track crosses the boundary twice");
        assert!(
            records.iter().all(|r| !r.inside_is_trace_end),
            "neither crossing is an end of the track, so neither would be filleted"
        );
    }

    #[test]
    fn a_sharp_entry_on_a_board_is_reported() {
        // The 41.410 degree case from the geometry tests, put on a board: a
        // 1.2mm trace leaving the centre of a 1.6mm round land radially.
        // 90 - asin(0.6 / 0.8) = 41.410, and the threshold is 45.
        let (mut world, net) = board_with_land(
            1.6,
            (0.0, 0.0),
            Rotation::ZERO,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        add_trace(
            &mut world,
            net,
            Layer::TopCopper,
            1.2,
            (10.0, 10.0),
            (10.0, 15.0),
        );

        let (violations, report) = measure_entries(&mut world);
        assert_eq!(report.examined, 1, "one segment enters the land");
        assert_eq!(report.refused, 0);
        assert_eq!(report.violations, 1);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].kind, crate::ViolationKind::PadEntry);
        assert!(
            violations[0].message.contains("J1.1") && violations[0].message.contains("41.4"),
            "the message names the pin and the angle: {}",
            violations[0].message
        );
        // An angle is not a distance and there is no field for it.
        assert_eq!(violations[0].actual, None);
        assert_eq!(violations[0].required, None);
    }

    #[test]
    fn a_clean_entry_is_examined_and_not_reported() {
        // The control that separates a clean board from an unexamined one.
        // The same land with a 0.25mm trace: 90 - asin(0.125 / 0.8) = 81.021.
        // An empty violation list proves nothing on its own; the denominator
        // does.
        let (mut world, net) = board_with_land(
            1.6,
            (0.0, 0.0),
            Rotation::ZERO,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        add_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.25,
            (10.0, 10.0),
            (10.0, 15.0),
        );

        let (violations, report) = measure_entries(&mut world);
        assert!(violations.is_empty(), "{violations:?}");
        assert_eq!(report.examined, 1, "it was looked at, and it was clean");
        assert_eq!(report.refused, 0);
        assert_eq!(report.violations, 0);
    }

    #[test]
    fn a_refused_entry_is_not_a_clean_one() {
        // A 2.0mm trace into a 1.6mm land: `TraceWiderThanLand`, which is
        // `NeckDownRule`'s question. The violation list is empty and the board
        // has not been cleared - that difference is the whole reason the
        // report exists.
        let (mut world, net) = board_with_land(
            1.6,
            (0.0, 0.0),
            Rotation::ZERO,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        add_trace(
            &mut world,
            net,
            Layer::TopCopper,
            2.0,
            (10.0, 10.0),
            (10.0, 15.0),
        );

        let (violations, report) = measure_entries(&mut world);
        assert!(violations.is_empty(), "{violations:?}");
        assert_eq!(report.examined, 1);
        assert_eq!(report.refused, 1, "measured nothing, and says so");
        assert_eq!(report.violations, 0);
    }

    #[test]
    fn the_segments_own_width_is_the_one_that_enters() {
        // The stretch that reaches a pad is exactly the one `neck` makes
        // thinner. The trace says 0.25mm and this segment says 1.2mm; reading
        // the trace's figure would report 81 degrees on a board that enters at
        // 41.410.
        let (mut world, net) = board_with_land(
            1.6,
            (0.0, 0.0),
            Rotation::ZERO,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        let mut trace = Trace::new(net);
        trace.layer = Layer::TopCopper;
        trace.width = Nm::from_mm(0.25);
        trace.segments.push(TraceSegment {
            start: Point::from_mm(10.0, 10.0),
            end: Point::from_mm(10.0, 15.0),
            width: Some(Nm::from_mm(1.2)),
        });
        world.spawn_entity((trace,));

        let (violations, report) = measure_entries(&mut world);
        assert_eq!(report.examined, 1);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].message.contains("41.4"),
            "the segment's width, not the trace's: {}",
            violations[0].message
        );
    }

    #[test]
    fn a_rotated_part_is_found_where_the_board_puts_it() {
        // The pad sits 1mm along x in the footprint and the part is turned a
        // quarter turn, so the land is at (10, 11) on the board. A walk that
        // ignored the rotation would look at (11, 10), find the trace's end
        // 1.414mm away from a land of radius 0.8mm, and examine nothing.
        let (mut world, net) = board_with_land(
            1.6,
            (1.0, 0.0),
            Rotation::DEG_90,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        add_trace(
            &mut world,
            net,
            Layer::TopCopper,
            1.2,
            (10.0, 11.0),
            (10.0, 16.0),
        );

        let (violations, report) = measure_entries(&mut world);
        assert_eq!(report.examined, 1, "the land is at (10, 11), not (11, 10)");
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].message.contains("41.4"),
            "{}",
            violations[0].message
        );
    }

    #[test]
    fn another_nets_copper_over_a_land_is_not_an_entry() {
        // That is a short, and `ClearanceRule` reports it. An entry angle for
        // it would answer a question nobody asked about a board already wrong.
        let (mut world, _net) = board_with_land(
            1.6,
            (0.0, 0.0),
            Rotation::ZERO,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        let other = world.intern_net("OTHER");
        add_trace(
            &mut world,
            other,
            Layer::TopCopper,
            1.2,
            (10.0, 10.0),
            (10.0, 15.0),
        );

        let (violations, report) = measure_entries(&mut world);
        assert!(violations.is_empty(), "{violations:?}");
        assert_eq!(report.examined, 0);
    }

    #[test]
    fn copper_on_a_layer_the_pad_is_not_on_is_not_an_entry() {
        // An SMD land on the top and its net's copper on the bottom do not
        // touch. They meet through a via, and the entry is wherever that via's
        // copper reaches the land - not here.
        let (mut world, net) =
            board_with_land(1.6, (0.0, 0.0), Rotation::ZERO, vec![Layer::TopCopper]);
        add_trace(
            &mut world,
            net,
            Layer::BottomCopper,
            1.2,
            (10.0, 10.0),
            (10.0, 15.0),
        );

        let (violations, report) = measure_entries(&mut world);
        assert!(violations.is_empty(), "{violations:?}");
        assert_eq!(report.examined, 0);
    }

    #[test]
    fn a_segment_with_both_ends_in_the_land_crosses_no_boundary() {
        // And one with both ends outside passes over the land rather than
        // entering it. Neither has a wedge; both would have one if the walk
        // asked "does this segment touch the pad" instead of "does it cross
        // out of it".
        let (mut world, net) = board_with_land(
            1.6,
            (0.0, 0.0),
            Rotation::ZERO,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        add_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.25,
            (9.8, 10.0),
            (10.2, 10.0),
        );
        add_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.25,
            (5.0, 10.0),
            (15.0, 10.0),
        );

        let (violations, report) = measure_entries(&mut world);
        assert!(violations.is_empty(), "{violations:?}");
        assert_eq!(report.examined, 0, "one stays inside, one passes over");
    }

    #[test]
    fn the_rule_is_in_the_registry_and_reports_what_the_walk_found() {
        let (mut world, net) = board_with_land(
            1.6,
            (0.0, 0.0),
            Rotation::ZERO,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        add_trace(
            &mut world,
            net,
            Layer::TopCopper,
            1.2,
            (10.0, 10.0),
            (10.0, 15.0),
        );

        assert_eq!(PadEntryRule.name(), "pad-entry");
        let violations = PadEntryRule.check(&mut world, &DesignRules::default());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].kind, crate::ViolationKind::PadEntry);

        // Registered, not merely written. `run_drc` builds the registry
        // itself, so this fails if the entry in `lib.rs` is missing - which is
        // the state R-08 sat in for four commits, geometry complete and
        // reporting nothing.
        let result = crate::run_drc(&mut world, &DesignRules::default());
        assert!(
            result
                .violations
                .iter()
                .any(|v| v.kind == crate::ViolationKind::PadEntry),
            "the registry has to carry it for it to fire: {:?}",
            result.violations.iter().map(|v| v.kind).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_end_in_the_land_is_the_one_the_wedge_is_measured_from() {
        // A round land entered radially cannot tell the two ends apart: the
        // land is symmetric about the trace, so starting from the far end and
        // travelling back through it leaves by the mirror crossing at the same
        // angle. Every other board-walk test here is that symmetric, and a
        // mutation that swapped the two ends survived all of them.
        //
        // This is the geometry that mutation cannot survive, and it is not
        // invented: it is what `uat-routing-locked.cypcb` shipped. A 0402 land
        // is 0.6 by 0.5mm, and a 0.2mm trace leaving its centre for a pad
        // 9mm right and 5mm down runs at atan(5 / 9) = 29.0546 degrees. The
        // upper edge leaves by the right side at 60.945; the lower edge
        // reaches the bottom side first and leaves at 29.055, and the answer
        // is the smaller of the two.
        let (mut world, net) = board_with_shaped_land(
            PadShape::Rect,
            (0.6, 0.5),
            (0.0, 0.0),
            Rotation::ZERO,
            vec![Layer::TopCopper, Layer::BottomCopper],
        );
        add_trace(
            &mut world,
            net,
            Layer::TopCopper,
            0.2,
            (10.0, 10.0),
            (19.0, 5.0),
        );

        let (violations, report) = measure_entries(&mut world);
        assert_eq!(report.examined, 1);
        assert_eq!(report.refused, 0);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].message.contains("29.1"),
            "29.055 degrees, printed to one decimal: {}",
            violations[0].message
        );
        // The wedge is reported where the copper meets the land, not at the
        // other end of a 10mm run.
        assert_eq!(violations[0].location, Point::from_mm(10.0, 10.0));
    }
}
