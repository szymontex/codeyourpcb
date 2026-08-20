/**
 * Two layers with a laminate between them do not get in each other's way.
 *
 * Reported by the owner: a trace being drawn on the top of the board refused
 * to cross a trace on the bottom. `checkRouteObstacles` compared nets and
 * never once asked which layer anything was on - the pad loop and the trace
 * loop both - so every piece of copper on the board was an obstacle to every
 * other piece regardless of which side it was on.
 */

import { describe, it, expect } from 'vitest';
import { checkRouteObstacles } from '../routing';
import type { BoardSnapshot } from '../types';

const MM = 1_000_000;

/** A pad on the layers its mask names, carrying a net. */
function pad(number: string, x: number, y: number, layerMask: number) {
  return {
    number,
    x_nm: x * MM,
    y_nm: y * MM,
    width_nm: 1 * MM,
    height_nm: 1 * MM,
    shape: 'rect',
    layer_mask: layerMask,
    drill_nm: null,
  };
}

/** A board with one part and one trace, each placed where the caller says. */
function board(padMask: number, traceLayer: string): BoardSnapshot {
  return {
    board: { width_nm: 40 * MM, height_nm: 20 * MM, layer_count: 2 },
    components: [
      {
        refdes: 'R9',
        x_nm: 20 * MM,
        y_nm: 10 * MM,
        rotation_mdeg: 0,
        pads: [pad('1', 0, 0, padMask)],
        silk: [],
      },
    ],
    traces: [
      {
        id: 1,
        net_name: 'OTHER',
        layer: traceLayer,
        width: 0.25 * MM,
        segments: [
          { start_x: 20 * MM, start_y: 4 * MM, end_x: 20 * MM, end_y: 16 * MM },
        ],
      },
    ],
    vias: [],
    ratsnest: [],
  } as unknown as BoardSnapshot;
}

/** A run straight through the middle of the board, west to east. */
const ACROSS = [
  { x: 5 * MM, y: 10 * MM },
  { x: 35 * MM, y: 10 * MM },
];

const TOP = 0x01;
const BOTTOM = 0x02;
const THROUGH_HOLE = TOP | BOTTOM;

const netMap = new Map([['R9.1', 'OTHER']]);

describe('copper on another layer is not in the way', () => {
  it('crosses a trace on the other side of the board', () => {
    const snapshot = board(BOTTOM, 'Bottom');
    const found = checkRouteObstacles(ACROSS, snapshot, 'MINE', 150_000, 250_000, netMap, 'Top');
    expect(found).toEqual([]);
  });

  it('still stops at a trace on its own layer', () => {
    const snapshot = board(BOTTOM, 'Top');
    const found = checkRouteObstacles(ACROSS, snapshot, 'MINE', 150_000, 250_000, netMap, 'Top');
    expect(found.length).toBeGreaterThan(0);
    expect(found.some((obstacle) => obstacle.type === 'trace')).toBe(true);
  });

  it('ignores a pad that has no copper on the layer being routed', () => {
    const snapshot = board(BOTTOM, 'Bottom');
    const found = checkRouteObstacles(ACROSS, snapshot, 'MINE', 150_000, 250_000, netMap, 'Top');
    expect(found.some((obstacle) => obstacle.type === 'pad')).toBe(false);
  });

  /**
   * A through-hole pad is a hole through every layer. Routing on the top of
   * the board does not make it disappear, and a router that thought otherwise
   * would drive copper straight through a drilled hole.
   */
  it('never ignores a through-hole pad', () => {
    const snapshot = board(THROUGH_HOLE, 'Bottom');
    const found = checkRouteObstacles(ACROSS, snapshot, 'MINE', 150_000, 250_000, netMap, 'Top');
    expect(found.some((obstacle) => obstacle.type === 'pad')).toBe(true);
  });

  /**
   * A caller with no layer to offer gets what it always got. Silently
   * reporting a clear board to code that has not been taught about layers
   * would turn this fix into a different bug.
   */
  it('checks everything when the caller names no layer', () => {
    const snapshot = board(BOTTOM, 'Bottom');
    const found = checkRouteObstacles(ACROSS, snapshot, 'MINE', 150_000, 250_000, netMap);
    expect(found.length).toBeGreaterThan(0);
  });
});
