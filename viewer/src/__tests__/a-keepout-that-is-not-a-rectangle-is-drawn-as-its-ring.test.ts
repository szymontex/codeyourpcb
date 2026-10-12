import { describe, it, expect } from 'vitest';
import { drawKeepout } from '../renderer';
import { worldToScreen } from '../viewport';
import type { ZoneInfo } from '../types';
import type { Viewport } from '../viewport';

/**
 * A keepout read from KiCad as an L is a ring of corners, and the notch of the
 * L is where a part may stand. Drawn as the box round it, the screen would
 * show the notch shut - the one thing the shape was drawn to say.
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

/** 10mm x 10mm with the upper right quarter cut away. */
const outline: [number, number][] = [
  [0, 0],
  [10_000_000, 0],
  [10_000_000, 5_000_000],
  [5_000_000, 5_000_000],
  [5_000_000, 10_000_000],
  [0, 10_000_000],
];

const ring: ZoneInfo = {
  name: 'antenna',
  kind: 'keepout',
  layer_mask: 0b01,
  net: '',
  bounds: [0, 0, 10_000_000, 10_000_000],
  outline,
};

const bothLayers = { topCopper: true, bottomCopper: true, topSilk: true, bottomSilk: true, drill: true };

describe('a keepout that is not a rectangle', () => {
  it('is drawn as its ring, one corner after the next, and closed', () => {
    const { ctx, calls } = recordingContext();
    drawKeepout(ctx, viewport, ring, bothLayers as never);

    const moves = calls.filter((c) => c.method === 'moveTo');
    const lines = calls.filter((c) => c.method === 'lineTo');
    expect(moves).toHaveLength(1);
    expect(lines).toHaveLength(outline.length - 1);
    expect(calls.filter((c) => c.method === 'closePath')).toHaveLength(1);
    expect(calls.some((c) => c.method === 'stroke')).toBe(true);

    const drawn = [...moves, ...lines].map((c) => c.args as number[]);
    const expected = outline.map(([x, y]) => worldToScreen(viewport, x, y));
    expect(drawn).toEqual(expected);
  });

  it('is not drawn as the box round it', () => {
    const { ctx, calls } = recordingContext();
    drawKeepout(ctx, viewport, ring, bothLayers as never);
    expect(calls.some((c) => c.method === 'strokeRect')).toBe(false);
  });

  it('is an outline and not a fill, dashed like any keepout', () => {
    const { ctx, calls } = recordingContext();
    drawKeepout(ctx, viewport, ring, bothLayers as never);
    expect(calls.some((c) => c.method === 'fill' || c.method === 'fillRect')).toBe(false);
    expect(calls.some((c) => c.method === 'setLineDash')).toBe(true);
  });

  it('is not drawn at all when its layers are hidden', () => {
    const { ctx, calls } = recordingContext();
    const hidden = { ...bothLayers, topCopper: false, bottomCopper: false };
    drawKeepout(ctx, viewport, ring, hidden as never);
    expect(calls.some((c) => c.method === 'stroke' || c.method === 'strokeRect')).toBe(false);
  });

  it('control: a keepout with no outline is still its box', () => {
    const { ctx, calls } = recordingContext();
    const { outline: _gone, ...rectangle } = ring;
    drawKeepout(ctx, viewport, rectangle, bothLayers as never);
    expect(calls.some((c) => c.method === 'strokeRect')).toBe(true);
    expect(calls.some((c) => c.method === 'lineTo')).toBe(false);
  });
});
