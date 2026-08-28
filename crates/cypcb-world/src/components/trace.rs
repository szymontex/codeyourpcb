//! Trace and via components for routing.
//!
//! These components represent copper traces and vias on the PCB.
//! Traces are polyline paths connecting pads, while vias are
//! drill holes that connect copper on different layers.
//!
//! # Architecture
//!
//! - [`TraceSegment`] - A single line segment (start to end point)
//! - [`Trace`] - A polyline path composed of segments, belonging to a net
//! - [`Via`] - A drill hole connecting layers, with annular ring
//! - [`TraceSource`] - Enum indicating if trace was manual or autorouted
//!
//! # Example
//!
//! ```
//! use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
//! use cypcb_world::{NetId, Layer};
//! use cypcb_core::{Nm, Point};
//!
//! // Create a trace with two segments
//! let segments = vec![
//!     TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(5.0, 0.0)),
//!     TraceSegment::new(Point::from_mm(5.0, 0.0), Point::from_mm(5.0, 5.0)),
//! ];
//!
//! let trace = Trace {
//!     segments,
//!     width: Nm::from_mm(0.2),
//!     layer: Layer::TopCopper,
//!     net_id: NetId::new(0),
//!     locked: false,
//!     source: TraceSource::Manual,
//! };
//!
//! assert_eq!(trace.segments.len(), 2);
//! ```

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use cypcb_core::{Nm, Point};

use crate::components::electrical::NetId;
use crate::components::physical::Layer;

/// A single line segment of a trace (from start to end point).
///
/// TraceSegment is not a component - it's a data structure used within [`Trace`].
/// Each segment represents a straight line between two points.
///
/// # Example
///
/// ```
/// use cypcb_world::components::trace::TraceSegment;
/// use cypcb_core::Point;
///
/// let seg = TraceSegment::new(
///     Point::from_mm(0.0, 0.0),
///     Point::from_mm(10.0, 5.0),
/// );
///
/// // Calculate segment length
/// let length = seg.length();
/// assert!(length.0 > 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceSegment {
    /// Start point of the segment.
    pub start: Point,
    /// End point of the segment.
    pub end: Point,
    /// The width this one segment runs at, when it is not the trace's.
    ///
    /// `None` means "whatever the trace says", which is what almost every
    /// segment means and why this is an option rather than a required field:
    /// a `Trace` that is one width end to end says so once.
    ///
    /// It exists because a real board is not one width end to end. A trace
    /// carrying amps has to be millimetres wide and a 2.54mm pad pitch has
    /// nowhere to put that, so the last stretch into a pad runs thin - which
    /// the language can already state as `neck 0.8mm for 4mm` and nothing
    /// could measure, because the copper's real geometry was not in the model.
    /// KiCad has always written a width per `(segment ...)`; this is where
    /// that survives being read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<Nm>,
}

impl TraceSegment {
    /// Create a new trace segment from start to end point.
    #[inline]
    pub fn new(start: Point, end: Point) -> Self {
        TraceSegment {
            start,
            end,
            width: None,
        }
    }

    /// Create a segment that runs at its own width rather than the trace's.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::TraceSegment;
    /// use cypcb_core::{Nm, Point};
    ///
    /// let neck = TraceSegment::new_with_width(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(4.0, 0.0),
    ///     Nm::from_mm(0.8),
    /// );
    /// assert_eq!(neck.width, Some(Nm::from_mm(0.8)));
    /// ```
    pub fn new_with_width(start: Point, end: Point, width: Nm) -> Self {
        TraceSegment {
            start,
            end,
            width: Some(width),
        }
    }

    /// Calculate the length of this segment in nanometers.
    ///
    /// Uses Euclidean distance formula: sqrt((x2-x1)^2 + (y2-y1)^2)
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::TraceSegment;
    /// use cypcb_core::{Nm, Point};
    ///
    /// // Horizontal segment, 10mm long
    /// let seg = TraceSegment::new(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 0.0),
    /// );
    /// assert_eq!(seg.length(), Nm::from_mm(10.0));
    ///
    /// // Vertical segment, 5mm long
    /// let seg2 = TraceSegment::new(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(0.0, 5.0),
    /// );
    /// assert_eq!(seg2.length(), Nm::from_mm(5.0));
    /// ```
    pub fn length(&self) -> Nm {
        let dx = self.end.x.0 - self.start.x.0;
        let dy = self.end.y.0 - self.start.y.0;

        // Use i128 to prevent overflow during squared calculation
        let dx_squared = (dx as i128) * (dx as i128);
        let dy_squared = (dy as i128) * (dy as i128);

        // sqrt of sum, rounded to nearest nanometer
        let sum = dx_squared + dy_squared;
        let length = (sum as f64).sqrt() as i64;

        Nm(length)
    }

    /// Get the midpoint of this segment.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::TraceSegment;
    /// use cypcb_core::Point;
    ///
    /// let seg = TraceSegment::new(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 10.0),
    /// );
    /// let mid = seg.midpoint();
    /// assert_eq!(mid, Point::from_mm(5.0, 5.0));
    /// ```
    pub fn midpoint(&self) -> Point {
        Point::new(
            Nm((self.start.x.0 + self.end.x.0) / 2),
            Nm((self.start.y.0 + self.end.y.0) / 2),
        )
    }
}

/// Cut one segment at `distance` from its start, returning the two halves.
///
/// The cut point is interpolated in i128 so a long segment on a large board
/// cannot overflow the multiply, and both halves inherit the original's own
/// width - the caller decides which of them the neck is.
fn split_at(segment: &TraceSegment, distance: i64) -> (TraceSegment, TraceSegment) {
    let length = segment.length().0.max(1);
    let dx = segment.end.x.0 as i128 - segment.start.x.0 as i128;
    let dy = segment.end.y.0 as i128 - segment.start.y.0 as i128;
    let at = Point::new(
        Nm(segment.start.x.0 + (dx * distance as i128 / length as i128) as i64),
        Nm(segment.start.y.0 + (dy * distance as i128 / length as i128) as i64),
    );
    (
        TraceSegment {
            start: segment.start,
            end: at,
            width: segment.width,
        },
        TraceSegment {
            start: at,
            end: segment.end,
            width: segment.width,
        },
    )
}

/// Indicates whether a trace was created manually or by an autorouter.
///
/// This is used to track the origin of traces for debugging and
/// to potentially apply different editing behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TraceSource {
    /// Trace was manually defined in the DSL.
    #[default]
    Manual,
    /// Trace was generated by the autorouter.
    Autorouted,
}

/// A copper trace connecting pads on a PCB.
///
/// Traces are polyline paths composed of one or more [`TraceSegment`]s.
/// Each trace belongs to a specific net and has a fixed width and layer.
///
/// # Fields
///
/// - `segments`: The polyline path as a vector of segments
/// - `width`: Trace width in nanometers
/// - `layer`: Which copper layer the trace is on
/// - `net_id`: Which net this trace belongs to
/// - `locked`: If true, autorouter should not modify this trace
/// - `source`: Whether trace was manual or autorouted
///
/// # Example
///
/// ```
/// use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
/// use cypcb_world::{NetId, Layer};
/// use cypcb_core::{Nm, Point};
///
/// let trace = Trace {
///     segments: vec![
///         TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 0.0)),
///     ],
///     width: Nm::from_mm(0.2),
///     layer: Layer::TopCopper,
///     net_id: NetId::new(0),
///     locked: true,
///     source: TraceSource::Manual,
/// };
///
/// assert!(trace.locked);
/// assert_eq!(trace.total_length(), Nm::from_mm(10.0));
/// ```
#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    /// The polyline path as a vector of connected segments.
    pub segments: Vec<TraceSegment>,
    /// Trace width in nanometers.
    pub width: Nm,
    /// Which copper layer this trace is on.
    pub layer: Layer,
    /// The net this trace belongs to.
    pub net_id: NetId,
    /// If true, autorouter should not modify this trace.
    pub locked: bool,
    /// Origin of this trace (manual or autorouted).
    pub source: TraceSource,
}

impl Trace {
    /// Create a new trace with default values.
    ///
    /// Creates an empty trace on the top copper layer with 0.2mm width.
    ///
    /// # Arguments
    ///
    /// * `net_id` - The net this trace belongs to
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::Trace;
    /// use cypcb_world::NetId;
    ///
    /// let trace = Trace::new(NetId::new(0));
    /// assert!(trace.segments.is_empty());
    /// assert!(!trace.locked);
    /// ```
    /// The longest unbroken stretch of copper narrower than the trace's width.
    ///
    /// This is what `neck <width> for <length>` bounds. The grammar says a
    /// neck is "how narrow the copper may get **on the way into a pad**, and
    /// how far it may run at that width" - one approach, not a budget spent
    /// across the board. A net whose copper reaches two pads through two
    /// branches necks twice and is obeying the declaration both times;
    /// [`Trace::necked_length`], which sums, would read that as twice the
    /// allowance and fail a board that is correct.
    ///
    /// A stretch stops at a wide segment and at a break between runs: two thin
    /// ends of two separate chains are two approaches, not one long one.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::{Trace, TraceNeck, TraceSegment};
    /// use cypcb_world::NetId;
    /// use cypcb_core::{Nm, Point};
    ///
    /// let mut trace = Trace::new(NetId::new(0));
    /// trace.width = Nm::from_mm(2.0);
    /// // Two separate chains, each necked at both of its ends.
    /// trace.segments.push(TraceSegment::new(
    ///     Point::from_mm(0.0, 0.0), Point::from_mm(20.0, 0.0)));
    /// trace.segments.push(TraceSegment::new(
    ///     Point::from_mm(0.0, 10.0), Point::from_mm(20.0, 10.0)));
    /// trace.apply_neck(TraceNeck { width: Nm::from_mm(0.8), length: Nm::from_mm(4.0) });
    ///
    /// // Four approaches: both ends of both chains.
    /// assert_eq!(trace.necked_length(), Nm::from_mm(16.0));
    /// assert_eq!(trace.longest_necked_stretch(), Nm::from_mm(4.0), "but never 16mm at once");
    /// ```
    pub fn longest_necked_stretch(&self) -> Nm {
        let mut longest = 0i64;
        for range in self.runs() {
            let mut current = 0i64;
            for segment in &self.segments[range] {
                if segment.width.is_some_and(|w| w.raw() < self.width.raw()) {
                    current += segment.length().0;
                    longest = longest.max(current);
                } else {
                    current = 0;
                }
            }
        }
        Nm(longest)
    }

    /// Draw a declared neck onto this trace's own geometry.
    ///
    /// `neck 0.8mm for 4mm` is a statement the language can make and could not
    /// draw: a trace synced from a design ran at one width end to end, so
    /// `NeckDownRule`'s fourth check - the copper against the claim - had
    /// nothing to measure on a board written here. This narrows the last
    /// `neck.length` of the run, splitting the segment the boundary falls
    /// inside so the join is where the width changes rather than wherever a
    /// vertex happened to be.
    ///
    /// **Both ends of every run are necked.** A `trace ... from A to B` lands
    /// on a pad going in and a pad coming out, and so does every chain the
    /// router lays: the copper is thin because *a* pad has no room for the
    /// wide copper, and a net usually meets two. `for <length>` bounds one
    /// approach rather than the board's whole allowance, which is what
    /// `Trace::longest_necked_stretch` measures and what the grammar means by
    /// "on the way into a pad".
    ///
    /// Two declarations are left as they are, because both are faults
    /// `NeckDownRule` reports and neither describes copper worth drawing: a
    /// neck no narrower than the trace, and a neck longer than the trace.
    /// Drawing either would turn a reported declaration fault into geometry
    /// that hides it.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::{Trace, TraceNeck, TraceSegment};
    /// use cypcb_world::NetId;
    /// use cypcb_core::{Nm, Point};
    ///
    /// let mut trace = Trace::new(NetId::new(0));
    /// trace.width = Nm::from_mm(2.0);
    /// trace.segments.push(TraceSegment::new(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(20.0, 0.0),
    /// ));
    ///
    /// trace.apply_neck(TraceNeck { width: Nm::from_mm(0.8), length: Nm::from_mm(4.0) });
    ///
    /// // Thin, wide, thin: a pad at each end of the run.
    /// assert_eq!(trace.segments.len(), 3);
    /// assert_eq!(trace.width_at(0), Nm::from_mm(0.8));
    /// assert_eq!(trace.width_at(1), Nm::from_mm(2.0));
    /// assert_eq!(trace.width_at(2), Nm::from_mm(0.8));
    /// assert_eq!(trace.necked_length(), Nm::from_mm(8.0), "4mm at each pad");
    /// assert_eq!(trace.longest_necked_stretch(), Nm::from_mm(4.0));
    /// ```
    pub fn apply_neck(&mut self, neck: TraceNeck) {
        if neck.width.raw() >= self.width.raw() || neck.length.raw() <= 0 {
            return;
        }

        // One `Trace` holds every segment a net has on a layer, and a net with
        // more than two pads branches: the list is a set of chains, not one
        // chain. A trace written in the language is one chain and this changes
        // nothing for it; copper the router laid can be several, and necking
        // the tail of the vector there would put thin copper in the middle of
        // the board where one branch ends and the next begins.
        //
        // Back to front, because narrowing a run inserts a segment and would
        // move every later run's indices.
        for range in self.runs().into_iter().rev() {
            self.neck_one_run(range, neck);
        }
    }

    /// The index ranges of this trace's contiguous runs.
    ///
    /// A run breaks where one segment's end is not the next one's start. The
    /// same rule `board_as_dsl` uses to decide where one `path` stops and the
    /// next begins - written once here rather than a second time there.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::{Trace, TraceSegment};
    /// use cypcb_world::NetId;
    /// use cypcb_core::Point;
    ///
    /// let mut trace = Trace::new(NetId::new(0));
    /// trace.segments.push(TraceSegment::new(
    ///     Point::from_mm(0.0, 0.0), Point::from_mm(1.0, 0.0)));
    /// trace.segments.push(TraceSegment::new(
    ///     Point::from_mm(1.0, 0.0), Point::from_mm(2.0, 0.0)));
    /// trace.segments.push(TraceSegment::new(
    ///     Point::from_mm(9.0, 9.0), Point::from_mm(9.0, 8.0)));
    ///
    /// assert_eq!(trace.runs(), vec![0..2, 2..3]);
    /// ```
    pub fn runs(&self) -> Vec<std::ops::Range<usize>> {
        let mut runs = Vec::new();
        let mut start = 0usize;
        for index in 1..self.segments.len() {
            if self.segments[index].start != self.segments[index - 1].end {
                runs.push(start..index);
                start = index;
            }
        }
        if start < self.segments.len() {
            runs.push(start..self.segments.len());
        }
        runs
    }

    /// Narrow both ends of one run, cutting the segments the boundaries fall in.
    ///
    /// **Both ends, because a run has a pad at each of them.** `trace SIG from
    /// R1.2 to R2.1` lands on a pad going in and a pad coming out, and so does
    /// every chain the router lays - the copper is thin because *a* pad has no
    /// room for the wide copper, and a net usually meets two. Necking one end
    /// left the other approach at full width on a board that had asked for it
    /// to be thin.
    ///
    /// The tail is narrowed first: a split there inserts a segment at an index
    /// past the head's working area, so the head's indices are untouched. That
    /// holds only because the two necks never meet, which is what the guard
    /// below enforces.
    ///
    /// A run with no room for two necks is left alone entirely. Half a
    /// treatment is not a smaller version of the right one - it would put thin
    /// copper at one arbitrary end of a run too short to carry the
    /// declaration, and `NeckDownRule` reports the declaration as the fault it
    /// is.
    fn neck_one_run(&mut self, range: std::ops::Range<usize>, neck: TraceNeck) {
        let run_length: i64 = self.segments[range.clone()]
            .iter()
            .map(|segment| segment.length().0)
            .sum();
        if neck.length.raw().saturating_mul(2) >= run_length {
            return;
        }

        self.neck_run_tail(range.clone(), neck);
        self.neck_run_head(range.start, neck);
    }

    /// Narrow backwards from the run's last segment.
    fn neck_run_tail(&mut self, range: std::ops::Range<usize>, neck: TraceNeck) {
        let mut remaining = neck.length.raw();
        let mut index = range.end;
        while index > range.start && remaining > 0 {
            index -= 1;
            let length = self.segments[index].length().0;
            if length <= 0 {
                continue;
            }
            if length <= remaining {
                self.segments[index].width = Some(neck.width);
                remaining -= length;
                continue;
            }

            // The boundary falls inside this segment: cut it there, and the
            // part nearer the run's far end is the neck.
            let (wide, thin) = split_at(&self.segments[index], length - remaining);
            self.segments[index] = wide;
            self.segments.insert(index + 1, thin);
            self.segments[index + 1].width = Some(neck.width);
            remaining = 0;
        }
    }

    /// Narrow forwards from the run's first segment.
    fn neck_run_head(&mut self, start: usize, neck: TraceNeck) {
        let mut remaining = neck.length.raw();
        let mut index = start;
        while index < self.segments.len() && remaining > 0 {
            let length = self.segments[index].length().0;
            if length <= 0 {
                index += 1;
                continue;
            }
            if length <= remaining {
                self.segments[index].width = Some(neck.width);
                remaining -= length;
                index += 1;
                continue;
            }

            // The boundary falls inside this segment: the part nearer the
            // run's first point is the neck, so the thin half comes first.
            let (thin, wide) = split_at(&self.segments[index], remaining);
            self.segments[index] = thin;
            self.segments[index].width = Some(neck.width);
            self.segments.insert(index + 1, wide);
            remaining = 0;
        }
    }

    /// The width one of this trace's segments actually runs at.
    ///
    /// A segment that states nothing runs at the trace's width. Out-of-range
    /// indices answer with the trace's width too, because a caller asking
    /// about a segment that is not there has a bug the width cannot fix.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::{Trace, TraceSegment};
    /// use cypcb_world::NetId;
    /// use cypcb_core::{Nm, Point};
    ///
    /// let mut trace = Trace::new(NetId::new(0));
    /// trace.width = Nm::from_mm(2.0);
    /// trace.segments.push(TraceSegment::new(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 0.0),
    /// ));
    /// trace.segments.push(TraceSegment::new_with_width(
    ///     Point::from_mm(10.0, 0.0),
    ///     Point::from_mm(14.0, 0.0),
    ///     Nm::from_mm(0.8),
    /// ));
    /// assert_eq!(trace.width_at(0), Nm::from_mm(2.0));
    /// assert_eq!(trace.width_at(1), Nm::from_mm(0.8));
    /// ```
    pub fn width_at(&self, index: usize) -> Nm {
        self.segments
            .get(index)
            .and_then(|segment| segment.width)
            .unwrap_or(self.width)
    }

    /// How far this trace runs narrower than its own width.
    ///
    /// This is the number `neck 0.8mm for 4mm` is a claim about, and until
    /// segments carried a width there was nothing to compare the claim
    /// against. A segment wider than the trace is not a neck and is not
    /// counted; a trace with no narrow segment answers zero.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::{Trace, TraceSegment};
    /// use cypcb_world::NetId;
    /// use cypcb_core::{Nm, Point};
    ///
    /// let mut trace = Trace::new(NetId::new(0));
    /// trace.width = Nm::from_mm(2.0);
    /// trace.segments.push(TraceSegment::new(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 0.0),
    /// ));
    /// trace.segments.push(TraceSegment::new_with_width(
    ///     Point::from_mm(10.0, 0.0),
    ///     Point::from_mm(14.0, 0.0),
    ///     Nm::from_mm(0.8),
    /// ));
    /// assert_eq!(trace.necked_length(), Nm::from_mm(4.0));
    /// ```
    pub fn necked_length(&self) -> Nm {
        let total: i64 = self
            .segments
            .iter()
            .filter(|segment| segment.width.is_some_and(|w| w.raw() < self.width.raw()))
            .map(|segment| segment.length().0)
            .sum();
        Nm(total)
    }

    pub fn new(net_id: NetId) -> Self {
        Trace {
            segments: Vec::new(),
            width: Nm::from_mm(0.2), // Default 0.2mm trace width
            layer: Layer::TopCopper,
            net_id,
            locked: false,
            source: TraceSource::Manual,
        }
    }

    /// Add a segment to this trace.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::{Trace, TraceSegment};
    /// use cypcb_world::NetId;
    /// use cypcb_core::Point;
    ///
    /// let mut trace = Trace::new(NetId::new(0));
    /// trace.add_segment(TraceSegment::new(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(5.0, 0.0),
    /// ));
    /// assert_eq!(trace.segments.len(), 1);
    /// ```
    pub fn add_segment(&mut self, segment: TraceSegment) {
        self.segments.push(segment);
    }

    /// Calculate the total length of this trace.
    ///
    /// Sums the lengths of all segments.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::{Trace, TraceSegment};
    /// use cypcb_world::NetId;
    /// use cypcb_core::{Nm, Point};
    ///
    /// let mut trace = Trace::new(NetId::new(0));
    /// trace.add_segment(TraceSegment::new(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 0.0),
    /// ));
    /// trace.add_segment(TraceSegment::new(
    ///     Point::from_mm(10.0, 0.0),
    ///     Point::from_mm(10.0, 5.0),
    /// ));
    ///
    /// // Total: 10mm + 5mm = 15mm
    /// assert_eq!(trace.total_length(), Nm::from_mm(15.0));
    /// ```
    pub fn total_length(&self) -> Nm {
        let total: i64 = self.segments.iter().map(|s| s.length().0).sum();
        Nm(total)
    }

    /// Get the start point of this trace (first segment start).
    ///
    /// Returns `None` if the trace has no segments.
    pub fn start_point(&self) -> Option<Point> {
        self.segments.first().map(|s| s.start)
    }

    /// Get the end point of this trace (last segment end).
    ///
    /// Returns `None` if the trace has no segments.
    pub fn end_point(&self) -> Option<Point> {
        self.segments.last().map(|s| s.end)
    }

    /// Check if this trace is empty (has no segments).
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Get the number of segments in this trace.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
}

/// A via connecting copper layers through a drill hole.
///
/// Vias are plated through-holes that connect traces on different
/// copper layers. They have a drill diameter and an outer diameter
/// (annular ring).
///
/// # Fields
///
/// - `position`: Center position of the via
/// - `drill`: Drill hole diameter in nanometers
/// - `outer_diameter`: Total via diameter including annular ring
/// - `start_layer`: Upper layer connection
/// - `end_layer`: Lower layer connection
/// - `net_id`: Which net this via belongs to
/// - `locked`: If true, autorouter should not modify this via
///
/// # Example
///
/// ```
/// use cypcb_world::components::trace::Via;
/// use cypcb_world::{NetId, Layer};
/// use cypcb_core::{Nm, Point};
///
/// let via = Via {
///     position: Point::from_mm(5.0, 5.0),
///     drill: Nm::from_mm(0.3),
///     outer_diameter: Nm::from_mm(0.6),
///     start_layer: Layer::TopCopper,
///     end_layer: Layer::BottomCopper,
///     net_id: NetId::new(0),
///     locked: false,
/// };
///
/// assert_eq!(via.annular_ring(), Nm::from_mm(0.15));
/// ```
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Via {
    /// Center position of the via.
    pub position: Point,
    /// Drill hole diameter in nanometers.
    pub drill: Nm,
    /// Total via diameter including annular ring.
    pub outer_diameter: Nm,
    /// Upper layer connection.
    pub start_layer: Layer,
    /// Lower layer connection.
    pub end_layer: Layer,
    /// The net this via belongs to.
    pub net_id: NetId,
    /// If true, autorouter should not modify this via.
    pub locked: bool,
}

impl Via {
    /// Create a new via with default sizes.
    ///
    /// Creates a via connecting top and bottom copper with:
    /// - 0.3mm drill diameter
    /// - 0.6mm outer diameter
    ///
    /// # Arguments
    ///
    /// * `position` - Center position of the via
    /// * `net_id` - The net this via belongs to
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::Via;
    /// use cypcb_world::NetId;
    /// use cypcb_core::Point;
    ///
    /// let via = Via::new(Point::from_mm(10.0, 10.0), NetId::new(0));
    /// assert!(!via.locked);
    /// ```
    pub fn new(position: Point, net_id: NetId) -> Self {
        Via {
            position,
            drill: Nm::from_mm(0.3),
            outer_diameter: Nm::from_mm(0.6),
            start_layer: Layer::TopCopper,
            end_layer: Layer::BottomCopper,
            net_id,
            locked: false,
        }
    }

    /// Calculate the annular ring width (distance from drill edge to via edge).
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::Via;
    /// use cypcb_world::NetId;
    /// use cypcb_core::{Nm, Point};
    ///
    /// let via = Via {
    ///     position: Point::from_mm(0.0, 0.0),
    ///     drill: Nm::from_mm(0.3),
    ///     outer_diameter: Nm::from_mm(0.6),
    ///     start_layer: cypcb_world::Layer::TopCopper,
    ///     end_layer: cypcb_world::Layer::BottomCopper,
    ///     net_id: NetId::new(0),
    ///     locked: false,
    /// };
    ///
    /// // Annular ring = (outer - drill) / 2 = (0.6 - 0.3) / 2 = 0.15mm
    /// assert_eq!(via.annular_ring(), Nm::from_mm(0.15));
    /// ```
    pub fn annular_ring(&self) -> Nm {
        Nm((self.outer_diameter.0 - self.drill.0) / 2)
    }

    /// Check if this via spans all copper layers (through-hole via).
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::components::trace::Via;
    /// use cypcb_world::{NetId, Layer};
    /// use cypcb_core::Point;
    ///
    /// let via = Via::new(Point::from_mm(0.0, 0.0), NetId::new(0));
    /// assert!(via.is_through_hole()); // Default is top-to-bottom
    /// ```
    pub fn is_through_hole(&self) -> bool {
        self.start_layer == Layer::TopCopper && self.end_layer == Layer::BottomCopper
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_segment_creation() {
        let seg = TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 5.0));

        assert_eq!(seg.start, Point::from_mm(0.0, 0.0));
        assert_eq!(seg.end, Point::from_mm(10.0, 5.0));
    }

    #[test]
    fn test_trace_segment_length_horizontal() {
        let seg = TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 0.0));

        assert_eq!(seg.length(), Nm::from_mm(10.0));
    }

    #[test]
    fn test_trace_segment_length_vertical() {
        let seg = TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(0.0, 5.0));

        assert_eq!(seg.length(), Nm::from_mm(5.0));
    }

    #[test]
    fn test_trace_segment_length_diagonal() {
        // 3-4-5 triangle: diagonal should be 5mm
        let seg = TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(3.0, 4.0));

        assert_eq!(seg.length(), Nm::from_mm(5.0));
    }

    #[test]
    fn test_trace_segment_midpoint() {
        let seg = TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 10.0));

        assert_eq!(seg.midpoint(), Point::from_mm(5.0, 5.0));
    }

    #[test]
    fn test_trace_creation() {
        let trace = Trace::new(NetId::new(42));

        assert!(trace.segments.is_empty());
        assert_eq!(trace.width, Nm::from_mm(0.2));
        assert_eq!(trace.layer, Layer::TopCopper);
        assert_eq!(trace.net_id, NetId::new(42));
        assert!(!trace.locked);
        assert_eq!(trace.source, TraceSource::Manual);
    }

    #[test]
    fn test_trace_add_segments() {
        let mut trace = Trace::new(NetId::new(0));

        trace.add_segment(TraceSegment::new(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(5.0, 0.0),
        ));
        trace.add_segment(TraceSegment::new(
            Point::from_mm(5.0, 0.0),
            Point::from_mm(5.0, 5.0),
        ));

        assert_eq!(trace.segment_count(), 2);
        assert!(!trace.is_empty());
    }

    #[test]
    fn test_trace_total_length() {
        let mut trace = Trace::new(NetId::new(0));

        trace.add_segment(TraceSegment::new(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 0.0),
        ));
        trace.add_segment(TraceSegment::new(
            Point::from_mm(10.0, 0.0),
            Point::from_mm(10.0, 5.0),
        ));

        // 10mm + 5mm = 15mm
        assert_eq!(trace.total_length(), Nm::from_mm(15.0));
    }

    #[test]
    fn test_trace_start_end_points() {
        let mut trace = Trace::new(NetId::new(0));

        trace.add_segment(TraceSegment::new(
            Point::from_mm(1.0, 2.0),
            Point::from_mm(3.0, 4.0),
        ));
        trace.add_segment(TraceSegment::new(
            Point::from_mm(3.0, 4.0),
            Point::from_mm(5.0, 6.0),
        ));

        assert_eq!(trace.start_point(), Some(Point::from_mm(1.0, 2.0)));
        assert_eq!(trace.end_point(), Some(Point::from_mm(5.0, 6.0)));
    }

    #[test]
    fn test_trace_empty() {
        let trace = Trace::new(NetId::new(0));

        assert!(trace.is_empty());
        assert_eq!(trace.start_point(), None);
        assert_eq!(trace.end_point(), None);
        assert_eq!(trace.total_length(), Nm(0));
    }

    #[test]
    fn test_via_creation() {
        let via = Via::new(Point::from_mm(10.0, 20.0), NetId::new(5));

        assert_eq!(via.position, Point::from_mm(10.0, 20.0));
        assert_eq!(via.drill, Nm::from_mm(0.3));
        assert_eq!(via.outer_diameter, Nm::from_mm(0.6));
        assert_eq!(via.start_layer, Layer::TopCopper);
        assert_eq!(via.end_layer, Layer::BottomCopper);
        assert_eq!(via.net_id, NetId::new(5));
        assert!(!via.locked);
    }

    #[test]
    fn test_via_annular_ring() {
        let via = Via {
            position: Point::from_mm(0.0, 0.0),
            drill: Nm::from_mm(0.3),
            outer_diameter: Nm::from_mm(0.6),
            start_layer: Layer::TopCopper,
            end_layer: Layer::BottomCopper,
            net_id: NetId::new(0),
            locked: false,
        };

        // (0.6 - 0.3) / 2 = 0.15mm
        assert_eq!(via.annular_ring(), Nm::from_mm(0.15));
    }

    #[test]
    fn test_via_is_through_hole() {
        let through_via = Via::new(Point::ORIGIN, NetId::new(0));
        assert!(through_via.is_through_hole());

        let blind_via = Via {
            position: Point::ORIGIN,
            drill: Nm::from_mm(0.2),
            outer_diameter: Nm::from_mm(0.4),
            start_layer: Layer::TopCopper,
            end_layer: Layer::Inner(1), // Not bottom
            net_id: NetId::new(0),
            locked: false,
        };
        assert!(!blind_via.is_through_hole());
    }

    #[test]
    fn test_trace_source_enum() {
        let manual = TraceSource::Manual;
        let auto = TraceSource::Autorouted;

        assert_ne!(manual, auto);
        assert_eq!(TraceSource::default(), TraceSource::Manual);
    }

    #[test]
    fn test_trace_locked() {
        let mut trace = Trace::new(NetId::new(0));
        trace.locked = true;

        assert!(trace.locked);
    }

    #[test]
    fn test_trace_with_different_layers() {
        let mut trace = Trace::new(NetId::new(0));
        trace.layer = Layer::BottomCopper;

        assert_eq!(trace.layer, Layer::BottomCopper);
    }

    #[test]
    fn test_trace_with_different_widths() {
        let mut trace = Trace::new(NetId::new(0));
        trace.width = Nm::from_mm(0.5);

        assert_eq!(trace.width, Nm::from_mm(0.5));
    }

    #[test]
    fn test_via_locked() {
        let mut via = Via::new(Point::ORIGIN, NetId::new(0));
        via.locked = true;

        assert!(via.locked);
    }

    #[test]
    fn test_trace_autorouted_source() {
        let mut trace = Trace::new(NetId::new(0));
        trace.source = TraceSource::Autorouted;

        assert_eq!(trace.source, TraceSource::Autorouted);
    }
}

/// The curve a run of copper was drawn as.
///
/// A `Trace` holds straight segments because everything that measures copper
/// here measures straight segments. An arc is flattened into those the moment
/// it is read, which is right for the checker and wrong for the writer: a
/// design that stated one curve came back as a dozen chords, and the next save
/// flattened the flattening.
///
/// This is the same shape as [`crate::components::Stitched`]: a marker
/// recording what a run of copper came from, so the writer can put the
/// sentence back rather than the copper it produced.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Curve {
    /// The centre the copper turns about.
    pub centre: Point,
    /// How far it turns, in millidegrees. Negative turns clockwise.
    pub sweep_millideg: i32,
}

/// How narrow a trace may get on the way into a pad, and for how far.
///
/// A separate component rather than a field on [`Trace`]: most traces do not
/// have one, and every literal `Trace { .. }` in this workspace's tests would
/// have to name it.
///
/// The length is what makes the width checkable. A trace carrying amps has to
/// be millimetres wide, and a 2.54mm pad pitch has nowhere to put that, so
/// every EDA lets the last stretch before a pad run thin - a short length of
/// copper does not have time to heat. Stating how far turns that from
/// something a reader has to trust into something the checker can measure.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceNeck {
    /// The narrow width the trace may run at.
    pub width: Nm,
    /// How far it may run at that width.
    pub length: Nm,
}
