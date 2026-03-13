import { describe, it, expect } from 'vitest';
import { hitTestTrace } from '../hit-test';
import type { BoardSnapshot, TraceInfo } from '../types';
import type { Viewport } from '../viewport';

/** Helper: build a minimal viewport centered at origin */
function makeViewport(opts?: Partial<Viewport>): Viewport {
  return {
    centerX: 0,
    centerY: 0,
    scale: 0.0001, // 1mm = 100px
    width: 800,
    height: 600,
    ...opts,
  };
}

/** Helper: build a minimal trace */
function makeTrace(segments: { sx: number; sy: number; ex: number; ey: number }[], width = 200_000, id = 1): TraceInfo {
  return {
    id,
    segments: segments.map(s => ({
      start_x: s.sx,
      start_y: s.sy,
      end_x: s.ex,
      end_y: s.ey,
    })),
    width,
    layer: 'F.Cu',
    net_name: 'GND',
    locked: false,
  };
}

/** Helper: build a snapshot from traces */
function makeSnapshot(traces: TraceInfo[]): BoardSnapshot {
  return {
    board: null,
    components: [],
    nets: [],
    violations: [],
    traces,
    vias: [],
    ratsnest: [],
  };
}

describe('hitTestTrace', () => {
  it('returns correct trace for point near a horizontal segment', () => {
    const vp = makeViewport();
    // Horizontal trace at world Y=0, from x=0 to x=10mm
    const trace = makeTrace([{ sx: 0, sy: 0, ex: 10_000_000, ey: 0 }]);
    const snapshot = makeSnapshot([trace]);

    // Click at screen center (maps to world 0,0 which is on the trace)
    const result = hitTestTrace(snapshot, vp, 400, 300, 5);
    expect(result).not.toBeNull();
    expect(result!.trace.id).toEqual(1);
    expect(result!.segmentIndex).toEqual(0);
  });

  it('returns null for point far from any trace', () => {
    const vp = makeViewport();
    // Trace at world origin, horizontal
    const trace = makeTrace([{ sx: 0, sy: 0, ex: 10_000_000, ey: 0 }]);
    const snapshot = makeSnapshot([trace]);

    // Click far away — top-left corner of screen, maps to world far from trace
    // screen (0,0) -> world: (-4_000_000, 3_000_000) — 3mm away from trace
    const result = hitTestTrace(snapshot, vp, 0, 0, 5);
    expect(result).toBeNull();
  });

  it('handles vertical segments', () => {
    const vp = makeViewport();
    // Vertical trace from (0,0) to (0, 10mm)
    const trace = makeTrace([{ sx: 0, sy: 0, ex: 0, ey: 10_000_000 }]);
    const snapshot = makeSnapshot([trace]);

    // Click at screen center (world 0,0) — on the trace start
    const result = hitTestTrace(snapshot, vp, 400, 300, 5);
    expect(result).not.toBeNull();
    expect(result!.trace.id).toEqual(1);
  });

  it('handles diagonal segments', () => {
    const vp = makeViewport();
    // 45-degree trace from (0,0) to (5mm, 5mm)
    const trace = makeTrace([{ sx: 0, sy: 0, ex: 5_000_000, ey: 5_000_000 }]);
    const snapshot = makeSnapshot([trace]);

    // Click at world origin (on the trace start)
    const result = hitTestTrace(snapshot, vp, 400, 300, 5);
    expect(result).not.toBeNull();
  });

  it('respects trace width — wider traces are easier to hit', () => {
    const vp = makeViewport();
    // Thin trace (0.1mm width) at world y=500_000 (0.5mm above center)
    // Distance from center = 0.5mm = 500_000nm
    // Tolerance in world = 5px / 0.0001 = 50_000nm
    // Hit radius = 50_000 + 50_000 (half of 0.1mm) = 100_000nm — too small for 500_000 distance
    const thinTrace = makeTrace([{ sx: -5_000_000, sy: 500_000, ex: 5_000_000, ey: 500_000 }], 100_000, 1);

    // Wide trace (2mm width) at same position
    // Hit radius = 50_000 + 1_000_000 (half of 2mm) = 1_050_000nm — enough for 500_000 distance
    const wideTrace = makeTrace([{ sx: -5_000_000, sy: 500_000, ex: 5_000_000, ey: 500_000 }], 2_000_000, 2);

    // With thin trace, click at center should miss
    const thinResult = hitTestTrace(makeSnapshot([thinTrace]), vp, 400, 300, 5);
    expect(thinResult).toBeNull();

    // With wide trace, click at center should hit
    const wideResult = hitTestTrace(makeSnapshot([wideTrace]), vp, 400, 300, 5);
    expect(wideResult).not.toBeNull();
    expect(wideResult!.trace.id).toEqual(2);
  });

  it('returns null for null snapshot', () => {
    const vp = makeViewport();
    const result = hitTestTrace(null, vp, 400, 300, 5);
    expect(result).toBeNull();
  });

  it('returns null for empty traces', () => {
    const vp = makeViewport();
    const result = hitTestTrace(makeSnapshot([]), vp, 400, 300, 5);
    expect(result).toBeNull();
  });
});
