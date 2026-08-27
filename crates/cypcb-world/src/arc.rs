//! A curve in copper, as chords the checker can already measure.
//!
//! Row 2 of the KiCad parity audit is that a track here cannot curve, and it
//! was deferred on 2026-08-27 with a measured reason: the DRC's clearance is
//! `segment_distance` over straight segments, and the router, the congestion
//! map and both interop paths are the same. An arc in the model would be
//! copper the checker cannot measure, which is worse than a board that cannot
//! curve.
//!
//! This is the half that removes the reason. An arc states a centre, a radius
//! and how far it turns; `flatten` hands back the chords that stand in for it,
//! and every measurement in the project already works on chords. Nothing here
//! reaches the language, the router or an exporter yet - a curve that can be
//! checked comes first.
//!
//! # The tolerance
//!
//! A chord cuts the corner. The worst error is the sagitta - the gap at the
//! middle of the chord - and it is `r(1 - cos(step/2))` for a step of that
//! angle. Ask for a tolerance and the step follows from it:
//!
//! ```text
//!   step = 2 * acos(1 - tolerance / radius)
//! ```
//!
//! The default is **10 microns**, which is finer than any fabricator's
//! registration - JLCPCB works to 0.05mm - and costs about one chord every
//! seven degrees on a 5mm radius. A number a house cannot hold to is a number
//! not worth carrying.
//!
//! # Which way it turns
//!
//! The sweep is signed: positive turns the way angles grow, counter-clockwise,
//! and negative turns the other way. A tool that drops the sign draws the
//! long way round a board, so the sign is carried rather than derived.

use cypcb_core::{Nm, Point};

/// A circular arc: where it is centred, how far out, and how far it turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arc {
    /// The centre it turns about.
    pub centre: Point,
    /// How far the copper is from that centre.
    pub radius: Nm,
    /// Where it starts, in millidegrees counter-clockwise from the `+X` axis.
    pub start_millideg: i32,
    /// How far it turns, in millidegrees. Negative turns clockwise.
    pub sweep_millideg: i32,
}

/// Millidegrees as radians.
fn radians(millideg: f64) -> f64 {
    millideg / 1000.0 * std::f64::consts::PI / 180.0
}

impl Arc {
    /// How far a chord may cut the corner unless somebody says otherwise.
    ///
    /// Ten microns. A fabricator's registration is five times that, so a
    /// finer figure buys accuracy nothing downstream can hold.
    pub const DEFAULT_TOLERANCE: Nm = Nm(10_000);

    /// The point at an angle on this arc's circle.
    fn point_at(&self, millideg: f64) -> Point {
        let angle = radians(millideg);
        Point {
            x: Nm(self.centre.x.0 + (self.radius.0 as f64 * angle.cos()).round() as i64),
            y: Nm(self.centre.y.0 + (self.radius.0 as f64 * angle.sin()).round() as i64),
        }
    }

    /// Where the copper starts.
    pub fn start(&self) -> Point {
        self.point_at(self.start_millideg as f64)
    }

    /// Where it stops.
    pub fn end(&self) -> Point {
        self.point_at((self.start_millideg + self.sweep_millideg) as f64)
    }

    /// How long the curve is, which is what a length match has to measure.
    pub fn length(&self) -> Nm {
        let sweep = radians(self.sweep_millideg as f64).abs();
        Nm((self.radius.0 as f64 * sweep).round() as i64)
    }

    /// How many chords this arc needs to stay inside `tolerance`.
    ///
    /// At least one, because two points is the least a curve can be drawn
    /// with, and the answer is a count rather than a step so that the chords
    /// come out evenly - an arc that ends on a short offcut has a corner in it
    /// where the copper should be smooth.
    pub fn chords(&self, tolerance: Nm) -> usize {
        let tolerance = if tolerance.0 <= 0 {
            Self::DEFAULT_TOLERANCE
        } else {
            tolerance
        };
        let sweep = radians(self.sweep_millideg as f64).abs();
        if self.radius.0 <= 0 || sweep <= 0.0 {
            return 1;
        }
        // A tolerance as coarse as the radius allows any step at all, so the
        // ratio is held inside the domain of acos rather than trusted.
        let ratio = (1.0 - tolerance.0 as f64 / self.radius.0 as f64).clamp(-1.0, 1.0);
        let step = 2.0 * ratio.acos();
        if step <= 0.0 {
            return 1;
        }
        (sweep / step).ceil().max(1.0) as usize
    }

    /// The worst gap between this arc and the chords that stand in for it.
    ///
    /// The number a person should be able to ask for rather than trust: a
    /// flattening is only as honest as the error it admits to.
    pub fn chord_error(&self, tolerance: Nm) -> Nm {
        let chords = self.chords(tolerance);
        let step = radians(self.sweep_millideg as f64).abs() / chords as f64;
        Nm((self.radius.0 as f64 * (1.0 - (step / 2.0).cos())).round() as i64)
    }

    /// The arc as the chords that stand in for it, ends included.
    ///
    /// Every measurement in this project - clearance, length, the congestion
    /// map, both exporters - works on straight segments between points. This
    /// is how a curve reaches all of them at once.
    pub fn flatten(&self, tolerance: Nm) -> Vec<Point> {
        if self.radius.0 <= 0 {
            return vec![self.centre];
        }
        if self.sweep_millideg == 0 {
            return vec![self.start()];
        }
        let chords = self.chords(tolerance);
        let mut points = Vec::with_capacity(chords + 1);
        for index in 0..=chords {
            let along = self.sweep_millideg as f64 * index as f64 / chords as f64;
            points.push(self.point_at(self.start_millideg as f64 + along));
        }
        points
    }
}
