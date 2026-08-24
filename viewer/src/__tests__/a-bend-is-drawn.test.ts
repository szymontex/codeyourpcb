import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { drawFlexRegion } from '../renderer';
import { LAYER_COLORS, createLayerVisibility } from '../layers';
import type { ZoneInfo } from '../types';
import type { Viewport } from '../viewport';

/**
 * The bend was the one thing a person could not see.
 *
 * The engine has sent `kind: "flex"` since rigid-flex shipped - `collect_zones`
 * says in its own comment that calling a flexible region a keepout "would have
 * turned every bend into an area nothing may enter" - and the screen drew
 * nothing for it. `renderer.ts` handled `keepout` and took its pours from a
 * different array, so a ribbon fell through both.
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

/** The ribbon of `examples/rigid-flex.cypcb`: 22mm to 38mm, full height. */
const bend: ZoneInfo = {
  name: 'bend',
  kind: 'flex',
  layer_mask: 0b11,
  net: '',
  bounds: [22_000_000, 0, 38_000_000, 16_000_000],
};

describe('the part of the board that bends', () => {
  it('is filled rather than outlined', () => {
    // The opposite of a keepout, and it says the opposite thing: a keepout is
    // an absence of copper, a flexible region is what the board is made of.
    const { ctx, calls } = recordingContext();
    drawFlexRegion(ctx, viewport, bend, createLayerVisibility());

    expect(calls.some((call) => call.method === 'fillRect')).toBe(true);
    expect(calls.some((call) => call.method === 'strokeRect')).toBe(true);
    expect(
      calls.some((call) => call.method === 'set:fillStyle' && call.args[0] === LAYER_COLORS.flex),
    ).toBe(true);
  });

  it('is faint, because copper crosses it', () => {
    // A ribbon carries traces - that is what it is for - so an opaque fill
    // would hide the thing it exists to support.
    const { ctx, calls } = recordingContext();
    drawFlexRegion(ctx, viewport, bend, createLayerVisibility());

    const alphas = calls
      .filter((call) => call.method === 'set:globalAlpha')
      .map((call) => call.args[0] as number);
    expect(alphas.length).toBeGreaterThan(0);
    expect(Math.min(...alphas)).toBeLessThan(0.5);
  });

  it('stays out of the way when neither face is being looked at', () => {
    const hidden = { ...createLayerVisibility(), topCopper: false, bottomCopper: false };
    const { ctx, calls } = recordingContext();
    drawFlexRegion(ctx, viewport, bend, hidden);

    expect(calls.length).toBe(0);
  });
});

describe('the renderer', () => {
  const source = readFileSync(join(__dirname, '..', 'renderer.ts'), 'utf8');

  it('dispatches on the kind the engine sends', () => {
    // The function is exported and testable; what a test cannot reach is the
    // loop that calls it, and a bend drawn by nothing is exactly what this
    // whole file is about.
    // The whole branch, not its two halves: a condition that names the kind
    // and then refuses to act on it reads as a dispatch and is not one.
    expect(source).toContain(
      "if (zone.kind === 'flex') {\n        drawFlexRegion(ctx, viewport, zone, layers);\n      }",
    );
  });

  it('draws it before the copper', () => {
    // It is what the board is made of there, so every piece of copper belongs
    // on top of it.
    const flexAt = source.indexOf("zone.kind === 'flex'");
    const keepoutAt = source.indexOf("zone.kind === 'keepout'");
    const poursAt = source.indexOf('drawPour(ctx, viewport, pour');
    expect(flexAt).toBeGreaterThan(-1);
    expect(flexAt).toBeLessThan(keepoutAt);
    expect(flexAt).toBeLessThan(poursAt);
  });
});
