import { describe, it, expect } from 'vitest';
import { WasmPcbEngineAdapter } from '../wasm';
import type { BoardSnapshot } from '../types';

/**
 * The browser and `cypcb check` have to give the same board the same answer.
 *
 * Two design rules were written twice: `silk-clearance` and `trace-current`
 * exist as Rust rules the engine runs, and existed again as TypeScript in this
 * module, whose results the adapter appended to the engine's. A board that
 * trips either got told about it twice, and the two copies had already drifted
 * - the Rust silk rule learned about printed designators and about clipping
 * the legend off copper, and the TypeScript one knew about neither.
 *
 * These tests hold the adapter to passing the engine's answer through.
 */

/** A board with one net, one trace and one violation the engine found. */
function snapshotWithViolation(): BoardSnapshot {
  return {
    board: { name: 't', width_nm: 20_000_000, height_nm: 20_000_000, layer_count: 2 },
    nets: [{ name: 'VCC', id: 1, current_ma: 2000 }],
    traces: [
      {
        id: 1,
        net_name: 'VCC',
        layer: 'top',
        width: 200_000,
        locked: false,
        segments: [
          { start_x: 1_000_000, start_y: 1_000_000, end_x: 5_000_000, end_y: 1_000_000 },
        ],
      },
    ],
    vias: [],
    ratsnest: [],
    components: [],
    violations: [
      {
        kind: 'trace-current',
        message: "trace 'VCC' is 0.20mm wide, 2000mA needs 0.55mm",
        x_nm: 1_000_000,
        y_nm: 1_000_000,
      },
    ],
  } as unknown as BoardSnapshot;
}

/** The smallest stand-in for the engine this test needs. */
function stubEngine(snapshot: BoardSnapshot) {
  return {
    get_snapshot: () => JSON.parse(JSON.stringify(snapshot)),
    get_min_clearance_nm: () => 130_000,
  } as never;
}

describe('the browser reports what the engine found, once', () => {
  it('does not add a second trace-current violation of its own', () => {
    const engine = new WasmPcbEngineAdapter(stubEngine(snapshotWithViolation()));
    const violations = engine.get_snapshot().violations ?? [];

    const current = violations.filter((v) => v.kind === 'trace-current');
    expect(current).toHaveLength(1);
  });

  it('passes the engine violations through unchanged', () => {
    const engine = new WasmPcbEngineAdapter(stubEngine(snapshotWithViolation()));
    const violations = engine.get_snapshot().violations ?? [];

    expect(violations).toHaveLength(1);
    expect(violations[0].message).toContain('2000mA needs');
  });
});
