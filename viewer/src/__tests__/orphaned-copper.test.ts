import { describe, it, expect } from 'vitest';
import { drawOrphanedCopper } from '../renderer';
import type { Viewport } from '../viewport';

/**
 * A pour island looks exactly like the rest of the plane, so the error list
 * can name it and the board still shows nothing. The violation carries the
 * sheet's rectangle and the canvas outlines it.
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

describe('copper a violation says is stranded', () => {
  it('is outlined, never filled over', () => {
    const { ctx, calls } = recordingContext();
    drawOrphanedCopper(ctx, viewport, [5_000_000, 5_000_000, 15_000_000, 15_000_000]);

    expect(calls.some((c) => c.method === 'strokeRect')).toBe(true);
    expect(calls.some((c) => c.method === 'fillRect' || c.method === 'fill')).toBe(false);
  });

  it('at the size of the sheet the checker reported', () => {
    const { ctx, calls } = recordingContext();
    drawOrphanedCopper(ctx, viewport, [5_000_000, 5_000_000, 15_000_000, 15_000_000]);

    const rect = calls.find((c) => c.method === 'strokeRect');
    const [, , width, height] = rect!.args as number[];
    // 10mm across at 1e-5 px/nm is 100 pixels.
    expect(width).toBeCloseTo(100, 5);
    expect(height).toBeCloseTo(100, 5);
  });
});
