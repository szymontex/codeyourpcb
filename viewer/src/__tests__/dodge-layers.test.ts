/**
 * The interactive router stopped bending around copper it cannot touch.
 *
 * Reported by the owner with two screenshots: instead of routing better, the
 * preview contorted and then still ended in a path it refused. Two causes -
 * the dodge built its obstacle rectangles from every pad on the board without
 * asking which layer any of them was on, and the obstacle list handed back to
 * be drawn was the one from before the dodge ran.
 */

import { describe, it, expect } from 'vitest';
import { dodgeObstacles } from '../dodge';
import type { BoardSnapshot } from '../types';

const MM = 1_000_000;
const TOP = 0x01;
const BOTTOM = 0x02;
const THROUGH_HOLE = TOP | BOTTOM;

/** One part sitting in the middle of a straight run, on the layers given. */
function boardWithPadAt(layerMask: number): BoardSnapshot {
  return {
    board: { width_nm: 40 * MM, height_nm: 20 * MM, layer_count: 2 },
    components: [
      {
        refdes: 'R9',
        x_nm: 20 * MM,
        y_nm: 10 * MM,
        rotation_mdeg: 0,
        pads: [
          {
            number: '1',
            x_nm: 0,
            y_nm: 0,
            width_nm: 2 * MM,
            height_nm: 2 * MM,
            shape: 'rect',
            layer_mask: layerMask,
            drill_nm: null,
          },
        ],
        silk: [],
      },
    ],
    traces: [],
    vias: [],
    ratsnest: [],
  } as unknown as BoardSnapshot;
}

/** Straight through where the pad sits. */
const STRAIGHT = [
  { x: 5 * MM, y: 10 * MM },
  { x: 35 * MM, y: 10 * MM },
];

const netMap = new Map([['R9.1', 'OTHER']]);

function dodge(mask: number, layer?: string) {
  return dodgeObstacles(STRAIGHT, boardWithPadAt(mask), 'MINE', 150_000, 250_000, netMap, layer);
}

describe('the dodge only avoids copper it could hit', () => {
  /** The complaint: a path bending around something on the other side. */
  it('runs straight past a pad on the other side of the board', () => {
    expect(dodge(BOTTOM, 'Top')).toEqual(STRAIGHT);
  });

  it('still bends around a pad on its own layer', () => {
    const bent = dodge(TOP, 'Top');
    expect(bent).not.toEqual(STRAIGHT);
    expect(bent.length).toBeGreaterThan(STRAIGHT.length);
  });

  /**
   * A through-hole pad is a hole through every layer. Routing on the top does
   * not make it go away, and a dodge that thought otherwise would drive
   * copper straight through a drilled hole.
   */
  it('never ignores a through-hole pad', () => {
    expect(dodge(THROUGH_HOLE, 'Top')).not.toEqual(STRAIGHT);
  });

  /** A caller with no layer to offer gets what it always got. */
  it('avoids everything when told nothing about layers', () => {
    expect(dodge(BOTTOM)).not.toEqual(STRAIGHT);
  });
});
