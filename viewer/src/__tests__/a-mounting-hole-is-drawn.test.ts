import { describe, it, expect } from 'vitest';
import { drawPad } from '../renderer';
import { getPadColor, createLayerVisibility, type LayerVisibility } from '../layers';
import type { PadInfo } from '../types';
import type { Viewport } from '../viewport';

/**
 * A mounting hole is a pad with a drill and no copper. Every drawing decision
 * in the viewer starts from the copper: `getPadColor` returns null when a pad
 * is on neither copper layer, and `drawPad` returns on a null colour. So the
 * board carried four 3.2mm holes, the drill file carried them, the router
 * refused to cross them, the checker measured copper against them - and the
 * screen showed bare laminate where each one was.
 *
 * That is worse than a hole drawn wrongly. A designer places parts by looking
 * at the board, and a hole nobody can see is a hole a connector gets placed
 * on top of.
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

const VIEWPORT: Viewport = {
  centerX: 10_000_000,
  centerY: 10_000_000,
  scale: 0.00002, // 20 screen px per millimetre
  width: 800,
  height: 600,
};

const BOTH_LAYERS_VISIBLE: LayerVisibility = {
  ...createLayerVisibility(),
  topCopper: true,
  bottomCopper: true,
};

/** An M3 mounting hole: 3.2mm drilled, on no copper layer. */
const MOUNTING_HOLE: PadInfo = {
  number: '',
  x_nm: 0,
  y_nm: 0,
  width_nm: 3_200_000,
  height_nm: 3_200_000,
  shape: 'circle',
  layer_mask: 0,
  drill_nm: 3_200_000,
};

/** An ordinary plated pin, for the half of this that must not change. */
const PLATED_PIN: PadInfo = {
  number: '1',
  x_nm: 0,
  y_nm: 0,
  width_nm: 1_700_000,
  height_nm: 1_700_000,
  shape: 'circle',
  layer_mask: 0x03,
  drill_nm: 1_000_000,
};

function draw(pad: PadInfo) {
  const { ctx, calls } = recordingContext();
  drawPad(
    ctx,
    VIEWPORT,
    10_000_000,
    10_000_000,
    0,
    pad,
    BOTH_LAYERS_VISIBLE,
    false,
    { background: '#1a1a1a' } as never,
    null,
    'H1',
    new Map(),
    'full' as never,
  );
  return calls;
}

describe('a mounting hole on screen', () => {
  it('is drawn, even though it is on no copper layer', () => {
    const calls = draw(MOUNTING_HOLE);

    const drew = calls.some((call) => call.method === 'fill' || call.method === 'stroke');
    expect(
      drew,
      'a 3.2mm hole in the board drew nothing at all, so the designer sees bare laminate',
    ).toBe(true);
  });

  it('is drawn at the size it is drilled', () => {
    const calls = draw(MOUNTING_HOLE);

    // 3.2mm at 20 screen px per millimetre is a 64px circle, so radius 32.
    const radii = calls
      .filter((call) => call.method === 'arc')
      .map((call) => call.args[2] as number);
    expect(radii.length, `no circle was drawn: ${JSON.stringify(calls)}`).toBeGreaterThan(0);
    expect(
      radii.some((r) => Math.abs(r - 32) < 1),
      `the hole is 3.2mm across, which is 32px of radius here; drew ${JSON.stringify(radii)}`,
    ).toBe(true);
  });

  it('is still drawn when neither copper layer is shown', () => {
    // A hole is mechanical. It is there whichever copper the designer is
    // looking at, and hiding the copper must not hide the hole - that is the
    // view somebody uses to check mechanical fit.
    const { ctx, calls } = recordingContext();
    drawPad(
      ctx,
      VIEWPORT,
      10_000_000,
      10_000_000,
      0,
      MOUNTING_HOLE,
      { ...BOTH_LAYERS_VISIBLE, topCopper: false, bottomCopper: false },
      false,
      { background: '#1a1a1a' } as never,
      null,
      'H1',
      new Map(),
      'full' as never,
    );
    expect(
      calls.some((call) => call.method === 'fill' || call.method === 'stroke'),
      'the hole vanished with the copper layers, and it is not on them',
    ).toBe(true);
  });

  it('does not change how a plated pin is drawn', () => {
    // The pin has copper, so it goes down the ordinary path: pad body first,
    // drill on top of it.
    const calls = draw(PLATED_PIN);
    const fills = calls.filter((call) => call.method === 'fill').length;
    expect(fills, 'a plated pin draws its copper and its hole').toBeGreaterThanOrEqual(2);
    expect(getPadColor(PLATED_PIN.layer_mask, BOTH_LAYERS_VISIBLE)).not.toBeNull();
  });
});
