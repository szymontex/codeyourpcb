import { describe, it, expect } from 'vitest';
import { drawKeepout } from '../renderer';
import type { ZoneInfo } from '../types';
import type { Viewport } from '../viewport';

/**
 * A keepout was carried, checked and routed around, and drawn by nothing. The
 * designer had to remember where they had put it.
 *
 * It is drawn as an outline rather than a fill on purpose: a keepout is an
 * absence of copper, and filling it would say the opposite of what it means.
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

const keepout: ZoneInfo = {
  name: 'mounting',
  kind: 'keepout',
  layer_mask: 0b11,
  net: '',
  bounds: [5_000_000, 5_000_000, 8_000_000, 8_000_000],
};

const bothLayers = { topCopper: true, bottomCopper: true, topSilk: true, bottomSilk: true, drill: true };

describe('a keepout is drawn', () => {
  it('as a dashed outline, not as a filled area', () => {
    const { ctx, calls } = recordingContext();
    drawKeepout(ctx, viewport, keepout, bothLayers as never);

    expect(calls.some((c) => c.method === 'strokeRect')).toBe(true);
    expect(calls.some((c) => c.method === 'fillRect' || c.method === 'fill')).toBe(false);
    expect(calls.some((c) => c.method === 'setLineDash')).toBe(true);
  });

  it('with the size the design gave it', () => {
    const { ctx, calls } = recordingContext();
    drawKeepout(ctx, viewport, keepout, bothLayers as never);

    const rect = calls.find((c) => c.method === 'strokeRect');
    const [, , width, height] = rect!.args as number[];
    // 3mm across at 1e-5 screen units per nm is 30 pixels.
    expect(width).toBeCloseTo(30, 5);
    expect(height).toBeCloseTo(30, 5);
  });

  it('and not at all when its layers are hidden', () => {
    const { ctx, calls } = recordingContext();
    const hidden = { ...bothLayers, topCopper: false, bottomCopper: false };
    drawKeepout(ctx, viewport, keepout, hidden as never);

    expect(calls.some((c) => c.method === 'strokeRect')).toBe(false);
  });
});
