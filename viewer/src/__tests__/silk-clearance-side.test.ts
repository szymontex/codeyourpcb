import { describe, it, expect } from 'vitest';
import { checkSilkClearance } from '../wasm';
import type { BoardSnapshot } from '../types';

/**
 * The JS silk check compares a shape's side against a pad's side. A shape from
 * the EasyEDA parser states one; a shape from the engine does not - a
 * footprint's artwork lives in footprint coordinates and the part decides
 * where it prints. Reading only the shape meant an engine-supplied legend
 * compared `undefined` against every pad, matched nothing, and skipped the
 * check in silence.
 */
describe('silk clearance against a legend that states no side', () => {
  const board: BoardSnapshot = {
    board: { name: 't', width_nm: 20_000_000, height_nm: 20_000_000, layer_count: 2 },
    nets: [],
    violations: [],
    traces: [],
    vias: [],
    ratsnest: [],
    components: [
      {
        refdes: 'R1',
        value: '10k',
        x_nm: 10_000_000,
        y_nm: 10_000_000,
        rotation_mdeg: 0,
        footprint: 'MARKED',
        pads: [
          { number: '1', x_nm: 0, y_nm: 0, width_nm: 600_000, height_nm: 500_000, shape: 'rect', layer_mask: 1, drill_nm: null },
        ],
        body_width_nm: 600_000,
        body_height_nm: 500_000,
        model_3d: null,
        // A line running straight across R2's pad, with no side stated.
        silk: [{ type: 'segment', x1: 0, y1: 0, x2: 2_000_000, y2: 0, width: 150_000 }],
      },
      {
        refdes: 'R2',
        value: '10k',
        x_nm: 12_000_000,
        y_nm: 10_000_000,
        rotation_mdeg: 0,
        footprint: 'MARKED',
        pads: [
          { number: '1', x_nm: 0, y_nm: 0, width_nm: 600_000, height_nm: 500_000, shape: 'rect', layer_mask: 1, drill_nm: null },
        ],
        body_width_nm: 600_000,
        body_height_nm: 500_000,
        model_3d: null,
        silk: [],
      },
    ],
  } as unknown as BoardSnapshot;

  it('still finds ink lying on another part pad', () => {
    const violations = checkSilkClearance(board, 130_000);
    expect(violations.length, 'silk over a foreign pad is a violation whoever supplied the shape').toBeGreaterThan(0);
    expect(violations[0].kind).toBe('silk-clearance');
  });
});
