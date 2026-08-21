/**
 * Net names along a trace, without two of them in the same place.
 *
 * Reported by the owner with a screenshot: `GND GND` printed twice within a
 * few pixels, and `GNDGND` overlapping at a bend. Two faults - a count that
 * was off by one, and no comparison between the labels of one segment and the
 * next.
 */

import { describe, it, expect } from 'vitest';
import { netLabelSpots, type ScreenSegment, type LabelLayout } from '../net-labels';

/** A viewport big enough that no segment here asks for a second name. */
const LAYOUT: LabelLayout = {
  minSegLenPx: 20,
  labelWidthPx: 30,
  viewWidth: 1400,
  viewHeight: 800,
};

const straight: ScreenSegment = { x1: 0, y1: 100, x2: 400, y2: 100 };

describe('where a net name goes', () => {
  /**
   * The count was `divisions = wanted + 1` and the loop placed `divisions` of
   * them, so one name became two a few character-widths apart.
   */
  it('places one name on a segment that asks for one', () => {
    const spots = netLabelSpots([straight], LAYOUT);
    expect(spots).toHaveLength(1);
    expect(spots[0].x).toBeCloseTo(200);
    expect(spots[0].y).toBeCloseTo(100);
  });

  it('says nothing on a segment too short to hold a name', () => {
    const stub: ScreenSegment = { x1: 0, y1: 0, x2: 10, y2: 0 };
    expect(netLabelSpots([stub], LAYOUT)).toEqual([]);
  });

  /**
   * The bend. Two segments meeting at a corner each want a name, and their
   * inner ends are close together - which is where `GNDGND` came from.
   */
  it('drops a name that would land on one already placed', () => {
    const corner: ScreenSegment[] = [
      { x1: 0, y1: 100, x2: 100, y2: 100 },
      // Doubles back, so its midpoint sits almost on the first one's.
      { x1: 100, y1: 100, x2: 4, y2: 100 },
    ];
    const spots = netLabelSpots(corner, LAYOUT);
    expect(spots).toHaveLength(1);
  });

  it('keeps both when the two segments are far enough apart', () => {
    const apart: ScreenSegment[] = [
      { x1: 0, y1: 100, x2: 200, y2: 100 },
      { x1: 600, y1: 100, x2: 800, y2: 100 },
    ];
    expect(netLabelSpots(apart, LAYOUT)).toHaveLength(2);
  });

  /** No two accepted spots are ever closer than one name's width. */
  it('never leaves two names within a name of each other', () => {
    const zigzag: ScreenSegment[] = [
      { x1: 0, y1: 0, x2: 120, y2: 0 },
      { x1: 120, y1: 0, x2: 120, y2: 60 },
      { x1: 120, y1: 60, x2: 40, y2: 60 },
      { x1: 40, y1: 60, x2: 40, y2: 5 },
    ];
    const spots = netLabelSpots(zigzag, LAYOUT);

    for (let i = 0; i < spots.length; i++) {
      for (let j = i + 1; j < spots.length; j++) {
        const gap = Math.hypot(spots[i].x - spots[j].x, spots[i].y - spots[j].y);
        expect(gap).toBeGreaterThanOrEqual(LAYOUT.labelWidthPx);
      }
    }
  });

  /** Text that would read upside down is turned the other way up. */
  it('keeps a name upright on a segment running right to left', () => {
    const backwards: ScreenSegment = { x1: 400, y1: 100, x2: 0, y2: 100 };
    const [spot] = netLabelSpots([backwards], LAYOUT);
    expect(Math.abs(spot.angle)).toBeLessThanOrEqual(Math.PI / 2);
  });

  /** A run long enough to cross the screen twice gets a second name. */
  it('repeats the name on a run that leaves the screen', () => {
    const long: ScreenSegment = { x1: 0, y1: 100, x2: 4200, y2: 100 };
    expect(netLabelSpots([long], LAYOUT).length).toBeGreaterThan(1);
  });
});
