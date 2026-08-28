import { describe, it, expect } from 'vitest';
import { tracePolyline } from '../renderer';
import type { TraceInfo } from '../types';
import type { Viewport } from '../viewport';

/**
 * The screen showed the flattening rather than the board.
 *
 * A curve reaches copper as the chords everything measures - the checker, the
 * router, the Gerbers - and the plots learned to draw the arc instead. The
 * canvas was the last place still drawing a dozen tiny facets on something the
 * design says is smooth, and at a high zoom that is what a person saw.
 */
function recordingContext() {
  const calls: { method: string; args: unknown[] }[] = [];
  const target: Record<string, unknown> = {};
  const ctx = new Proxy(target, {
    get(_t, prop: string) {
      if (prop in target) return target[prop];
      return (...args: unknown[]) => {
        calls.push({ method: prop, args });
      };
    },
    set(_t, prop: string, value) {
      target[prop] = value;
      calls.push({ method: `set:${prop}`, args: [value] });
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
  return { ctx, calls };
}

/** One screen pixel to 100 microns, centred on the curve of `curved-track`. */
const viewport: Viewport = {
  centerX: 12_000_000,
  centerY: 10_000_000,
  scale: 1e-5,
  width: 800,
  height: 600,
};

/** The quarter turn of `examples/curved-track.cypcb`, clockwise. */
const curved: TraceInfo = {
  id: 1,
  segments: [
    { start_x: 12_000_000, start_y: 6_000_000, end_x: 11_500_000, end_y: 6_030_000 },
    { start_x: 11_500_000, start_y: 6_030_000, end_x: 8_000_000, end_y: 10_000_000 },
  ],
  width: 250_000,
  layer: 'Top',
  net_name: 'SIG',
  locked: false,
  curve: {
    centre_x: 12_000_000,
    centre_y: 10_000_000,
    radius: 4_000_000,
    start_degrees: -90,
    sweep_degrees: -90,
  },
};

/** The same copper with nothing said about a curve. */
const straight: TraceInfo = { ...curved, curve: null };

describe('copper the board states as a curve', () => {
  it('is drawn as one arc rather than as its chords', () => {
    const { ctx, calls } = recordingContext();
    tracePolyline(ctx, viewport, curved);

    const arcs = calls.filter((call) => call.method === 'arc');
    expect(arcs).toHaveLength(1);
    expect(calls.filter((call) => call.method === 'lineTo')).toHaveLength(0);
  });

  it('turns about the centre the board states, at the radius it turns at', () => {
    const { ctx, calls } = recordingContext();
    tracePolyline(ctx, viewport, curved);

    const [cx, cy, r] = calls.find((call) => call.method === 'arc')!.args as number[];
    // The centre is the middle of the viewport, and 4mm is 40 pixels here.
    expect(cx).toBeCloseTo(400, 6);
    expect(cy).toBeCloseTo(300, 6);
    expect(r).toBeCloseTo(40, 6);
  });

  it('turns the way the board says, in the screen own direction', () => {
    // Screen Y grows down, so a board angle is its own negative on the canvas
    // and a turn that grows the angle on the board shrinks it on the screen.
    // A curve drawn the other way round is copper on the far side of the pad
    // it was meant to reach.
    const { ctx, calls } = recordingContext();
    tracePolyline(ctx, viewport, curved);
    const [, , , start, end, anticlockwise] = calls.find((call) => call.method === 'arc')!
      .args as [number, number, number, number, number, boolean];

    expect(start).toBeCloseTo(Math.PI / 2, 6);
    expect(end).toBeCloseTo(Math.PI, 6);
    expect(anticlockwise).toBe(false);

    const widdershins = { ...curved, curve: { ...curved.curve!, sweep_degrees: 90 } };
    const second = recordingContext();
    tracePolyline(second.ctx, viewport, widdershins);
    const turned = second.calls.find((call) => call.method === 'arc')!.args as unknown[];
    expect(turned[5]).toBe(true);
    expect(turned[4] as number).toBeCloseTo(0, 6);
  });

  it('leaves copper that is not a curve exactly as it was', () => {
    const { ctx, calls } = recordingContext();
    tracePolyline(ctx, viewport, straight);

    expect(calls.filter((call) => call.method === 'arc')).toHaveLength(0);
    expect(calls.filter((call) => call.method === 'lineTo')).toHaveLength(2);
    expect(calls.filter((call) => call.method === 'moveTo')).toHaveLength(1);
  });
});
