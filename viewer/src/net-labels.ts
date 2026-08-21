/**
 * Where a net's name goes along its copper.
 *
 * KiCad prints a net name repeatedly along a trace so you can read what a run
 * carries without following it to a pad. Two things were wrong with the way
 * this project copied that.
 *
 * The count was off by one. `divisions = numNames + 1` and then a loop over
 * `divisions` positions, so a segment that asked for one label got two, a few
 * character-widths apart - the `GND GND` the owner photographed.
 *
 * And every segment placed its labels alone. At a bend, the last label of one
 * segment and the first of the next land within a few pixels of each other and
 * overlap into `GNDGND`. Nothing compared one placement against another.
 */

/** A segment of a trace, in screen pixels. */
export interface ScreenSegment {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

/** Where one copy of the name is drawn, and which way it faces. */
export interface LabelSpot {
  x: number;
  y: number;
  angle: number;
}

export interface LabelLayout {
  /** A segment shorter than this carries no label at all. */
  minSegLenPx: number;
  /** How wide the name is when drawn, which is the spacing floor. */
  labelWidthPx: number;
  viewWidth: number;
  viewHeight: number;
}

/**
 * How many copies of the name a segment of this length and direction wants.
 *
 * The viewport is the yardstick, as it is in KiCad: a run that crosses the
 * screen gets a second name so one is always in view.
 */
function namesAlong(segLen: number, dx: number, dy: number, layout: LabelLayout): number {
  if (Math.abs(dy) < 1) return Math.max(1, Math.round(segLen / layout.viewWidth));
  if (Math.abs(dx) < 1) return Math.max(1, Math.round(segLen / layout.viewHeight));
  const minDim = Math.min(layout.viewWidth, layout.viewHeight);
  return Math.max(1, Math.round(segLen / (Math.SQRT2 * minDim)));
}

/** Text that would read upside down is turned the other way up. */
function uprightAngle(dx: number, dy: number): number {
  let angle = Math.atan2(dy, dx);
  if (angle > Math.PI / 2) angle -= Math.PI;
  if (angle < -Math.PI / 2) angle += Math.PI;
  return angle;
}

/**
 * Every place this trace's name should be drawn, already thinned out.
 *
 * Placements are checked against the ones already accepted for the same trace,
 * across segments rather than within one, because a bend is exactly where two
 * segments each put a label near the corner they share.
 */
export function netLabelSpots(
  segments: readonly ScreenSegment[],
  layout: LabelLayout,
): LabelSpot[] {
  const spots: LabelSpot[] = [];
  // A name has to clear its own width, or two of them touch. Half of it is
  // enough between the centres of two names running the same way, but a bend
  // puts them at an angle to each other - so the whole width is the honest
  // floor and costs at most one label on a long straight run.
  const minGap = Math.max(1, layout.labelWidthPx);

  for (const seg of segments) {
    const dx = seg.x2 - seg.x1;
    const dy = seg.y2 - seg.y1;
    const segLen = Math.hypot(dx, dy);
    if (segLen < layout.minSegLenPx) continue;

    const wanted = namesAlong(segLen, dx, dy, layout);
    const angle = uprightAngle(dx, dy);

    // `wanted` names means `wanted` names. This read `divisions = wanted + 1`
    // and then placed `divisions` of them.
    for (let i = 1; i <= wanted; i++) {
      const t = i / (wanted + 1);
      const x = seg.x1 + dx * t;
      const y = seg.y1 + dy * t;

      const crowded = spots.some(
        (placed) => Math.hypot(placed.x - x, placed.y - y) < minGap,
      );
      if (crowded) continue;

      spots.push({ x, y, angle });
    }
  }

  return spots;
}
