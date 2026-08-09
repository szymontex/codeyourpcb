import { describe, it, expect } from 'vitest';
import { drawBoardOutline } from '../renderer';

import type { Viewport } from '../viewport';

/**
 * A board is what its outline says, not what its size says.
 *
 * `outline { point ... }` has been in the language for a long time. The
 * checker measures edge clearance against it, the Gerber exporter cuts to it,
 * and this screen drew the rectangle `size` describes whatever shape the board
 * really was - so a board with a slot or a cutout looked like a plain
 * rectangle here and arrived at the fab as something else.
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

/** `getThemeColors()` reads the page's CSS variables, and there is no page here. */
const colors = { board_outline: '#f0e14a' } as never;

const viewport: Viewport = {
  centerX: 20_000_000,
  centerY: 15_000_000,
  scale: 1e-5,
  width: 800,
  height: 600,
};

/** The U-shaped board of `examples/cutout.cypcb`, in nanometres. */
const CUTOUT: Array<[number, number]> = [
  [0, 0],
  [40_000_000, 0],
  [40_000_000, 30_000_000],
  [25_000_000, 30_000_000],
  [25_000_000, 10_000_000],
  [15_000_000, 10_000_000],
  [15_000_000, 30_000_000],
  [0, 30_000_000],
];

describe('the board is drawn as the shape it is', () => {
  it('follows a stated outline point by point', () => {
    const { ctx, calls } = recordingContext();

    drawBoardOutline(ctx, viewport, 40_000_000, 30_000_000, colors, CUTOUT);

    const moves = calls.filter(c => c.method === 'moveTo');
    const lines = calls.filter(c => c.method === 'lineTo');
    expect(moves).toHaveLength(1);
    expect(lines).toHaveLength(CUTOUT.length - 1);
    expect(calls.some(c => c.method === 'closePath')).toBe(true);

    // The slot's inner corner has to be on the path, or the shape drawn is not
    // the shape the design states.
    const corner = lines.some(c => {
      const [x, y] = c.args as [number, number];
      return Math.abs(x - 450) < 1 && Math.abs(y - 350) < 1;
    });
    expect(corner, `the point 25mm,10mm is missing: ${JSON.stringify(lines.map(l => l.args))}`).toBe(true);
  });

  it('does not fall back to a rectangle when it has an outline', () => {
    const { ctx, calls } = recordingContext();

    drawBoardOutline(ctx, viewport, 40_000_000, 30_000_000, colors, CUTOUT);

    expect(calls.some(c => c.method === 'fillRect')).toBe(false);
    expect(calls.some(c => c.method === 'strokeRect')).toBe(false);
  });

  it('draws the size rectangle when the design states no outline', () => {
    // Which is every board that does not need a shape, and most boards do not.
    const { ctx, calls } = recordingContext();

    drawBoardOutline(ctx, viewport, 40_000_000, 30_000_000, colors, undefined);

    expect(calls.some(c => c.method === 'fillRect')).toBe(true);
    expect(calls.some(c => c.method === 'strokeRect')).toBe(true);
    expect(calls.some(c => c.method === 'lineTo')).toBe(false);
  });

  it('ignores an outline too short to be a shape', () => {
    const { ctx, calls } = recordingContext();

    drawBoardOutline(ctx, viewport, 40_000_000, 30_000_000, colors, [
      [0, 0],
      [1_000_000, 0],
    ]);

    expect(calls.some(c => c.method === 'fillRect')).toBe(true);
  });
});
