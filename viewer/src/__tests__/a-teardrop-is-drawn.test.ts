import { describe, it, expect } from 'vitest';
import { drawTeardrop } from '../renderer';
import { createLayerVisibility } from '../layers';
import type { TeardropInfo } from '../types';
import type { Viewport } from '../viewport';

/**
 * A teardrop is copper, and the screen shows it.
 *
 * The engine sends each teardrop as the polygon the checker measures and the
 * Gerber carries, so the screen shows the copper the fabricator receives. A
 * renderer that drew traces, pours and vias and nothing for this would show a
 * board without the fillet its files carry.
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

const viewport: Viewport = {
  centerX: 0,
  centerY: 0,
  scale: 1e-5,
  width: 800,
  height: 600,
};

const fillet: TeardropInfo = {
  net: 'SIG',
  layer_mask: 0b01,
  points: [
    [0, 0],
    [800_000, 200_000],
    [800_000, -200_000],
    [0, -400_000],
  ],
};

describe('a teardrop on the screen', () => {
  it('is one filled polygon through every corner', () => {
    const { ctx, calls } = recordingContext();
    drawTeardrop(ctx, viewport, fillet, createLayerVisibility(), null);

    expect(calls.filter((call) => call.method === 'moveTo')).toHaveLength(1);
    expect(calls.filter((call) => call.method === 'lineTo')).toHaveLength(3);
    expect(calls.some((call) => call.method === 'closePath')).toBe(true);
    expect(calls.some((call) => call.method === 'fill')).toBe(true);
  });

  it('is the colour of the layer it is on', () => {
    const top = recordingContext();
    drawTeardrop(top.ctx, viewport, fillet, createLayerVisibility(), null);
    const bottom = recordingContext();
    drawTeardrop(
      bottom.ctx,
      viewport,
      { ...fillet, layer_mask: 0b10 },
      createLayerVisibility(),
      null,
    );
    const colour = (calls: { method: string; args: unknown[] }[]) =>
      calls.find((call) => call.method === 'set:fillStyle')?.args[0];
    expect(colour(top.calls)).toBeDefined();
    expect(colour(top.calls)).not.toEqual(colour(bottom.calls));
  });

  it('is left out where its layer is hidden', () => {
    const layers = createLayerVisibility();
    layers.topCopper = false;
    const { ctx, calls } = recordingContext();
    drawTeardrop(ctx, viewport, fillet, layers, null);
    expect(calls.some((call) => call.method === 'fill')).toBe(false);
  });

  it('fades with the net that is not highlighted', () => {
    const lit = recordingContext();
    drawTeardrop(lit.ctx, viewport, fillet, createLayerVisibility(), 'SIG');
    const dim = recordingContext();
    drawTeardrop(dim.ctx, viewport, fillet, createLayerVisibility(), 'OTHER');
    const colour = (calls: { method: string; args: unknown[] }[]) =>
      calls.find((call) => call.method === 'set:fillStyle')?.args[0];
    expect(colour(dim.calls)).not.toEqual(colour(lit.calls));
  });

  it('is nothing at fewer than three corners', () => {
    const { ctx, calls } = recordingContext();
    drawTeardrop(
      ctx,
      viewport,
      { ...fillet, points: [[0, 0], [1, 1]] },
      createLayerVisibility(),
      null,
    );
    expect(calls.some((call) => call.method === 'fill')).toBe(false);
  });
});
