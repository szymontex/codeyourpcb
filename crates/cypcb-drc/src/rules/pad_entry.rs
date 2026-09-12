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

use cypcb_core::{Nm, Point};
use cypcb_world::footprint::PadDef;
use cypcb_world::{Pad, PadShape};

use super::rotate_point;

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
    let offset = rotate_point(pad.position, rotation_deg);
    let centre = Point::from_raw(at.x.raw() + offset.x.raw(), at.y.raw() + offset.y.raw());
    let into_pad = |p: Point| {
        rotate_point(
            Point::from_raw(p.x.raw() - centre.x.raw(), p.y.raw() - centre.y.raw()),
            -rotation_deg,
        )
    };
    entry_angle(
        PadOutline {
            shape: pad.shape,
            width: pad.size.0,
            height: pad.size.1,
        },
        into_pad(end),
        into_pad(arm),
        width,
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
}
