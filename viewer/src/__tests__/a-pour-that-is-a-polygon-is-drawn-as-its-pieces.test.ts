import { describe, it, expect } from 'vitest';
import { drawPour } from '../renderer';
import { worldToScreen } from '../viewport';
import type { PourInfo } from '../types';
import type { Viewport } from '../viewport';

/**
 * A pour drawn as an L arrives as convex pieces, each a ring of corners. They
 * are filled one after the next, so the screen shows the L and not the box
 * round it.
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

const viewport: Viewport = { centerX: 0, centerY: 0, scale: 1e-5, width: 800, height: 600 };
const layers = { topCopper: true, bottomCopper: true, topSilk: true, bottomSilk: true, drill: true };

/** The two convex pieces of an L: the foot and the stem above it, in nm. */
const foot: [number, number][] = [[0, 0], [10_000_000, 0], [10_000_000, 5_000_000], [0, 5_000_000]];
const stem: [number, number][] = [[0, 5_000_000], [5_000_000, 5_000_000], [5_000_000, 10_000_000], [0, 10_000_000]];

const polygonPour: PourInfo = { net: 'GND', layer_mask: 0b01, rects: [], polygons: [foot, stem] };
const rectPour: PourInfo = { net: 'GND', layer_mask: 0b01, rects: [[0, 0, 10_000_000, 10_000_000]] };

describe('a pour that is a polygon', () => {
  it('is filled piece by piece, each as a closed ring of its own corners', () => {
    const { ctx, calls } = recordingContext();
    drawPour(ctx, viewport, polygonPour, layers as never, null);

    expect(calls.filter((c) => c.method === 'beginPath')).toHaveLength(2);
    expect(calls.filter((c) => c.method === 'closePath')).toHaveLength(2);
    expect(calls.filter((c) => c.method === 'fill')).toHaveLength(2);
    const points = calls
      .filter((c) => c.method === 'moveTo' || c.method === 'lineTo')
      .map((c) => c.args as number[]);
    const expected = [...foot, ...stem].map(([x, y]) => worldToScreen(viewport, x, y));
    expect(points).toEqual(expected);
  });

  it('is not drawn as rectangles', () => {
    const { ctx, calls } = recordingContext();
    drawPour(ctx, viewport, polygonPour, layers as never, null);
    expect(calls.some((c) => c.method === 'fillRect')).toBe(false);
  });

  it('is not drawn when its layer is hidden', () => {
    const { ctx, calls } = recordingContext();
    drawPour(ctx, viewport, polygonPour, { ...layers, topCopper: false } as never, null);
    expect(calls.some((c) => c.method === 'fill' || c.method === 'fillRect')).toBe(false);
  });

  it('control: a rectangle pour is still its rectangles', () => {
    const { ctx, calls } = recordingContext();
    drawPour(ctx, viewport, rectPour, layers as never, null);
    expect(calls.filter((c) => c.method === 'fillRect')).toHaveLength(1);
    expect(calls.some((c) => c.method === 'fill')).toBe(false);
  });
});
