import { describe, it, expect } from 'vitest';
import { drawPad } from '../renderer';
import { createLayerVisibility, type LayerVisibility } from '../layers';
import type { PadInfo } from '../types';
import type { Viewport } from '../viewport';

/**
 * A slot is milled along its length, not drilled. The files say so now - the
 * importer keeps both of its dimensions and the drill file mills it with a
 * `G85` path - and the screen was the last place that still disagreed.
 *
 * The renderer drew every hole as a circle of the pad's drill number, which
 * for a slot is its *narrow* dimension. So a 2.4x1.0mm slot, the kind a USB
 * connector or a barrel jack holds itself down with, appeared as a 1mm round
 * hole. A designer places parts by looking at the board: a hole drawn at less
 * than half its length is a hole they will route a trace across.
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

/** A latching connector's anchor: 2.4mm long, 1.0mm wide. */
const SLOTTED_PAD: PadInfo = {
  number: '2',
  x_nm: 0,
  y_nm: 0,
  width_nm: 3_200_000,
  height_nm: 1_800_000,
  shape: 'oblong',
  layer_mask: 0x03,
  drill_nm: 1_000_000,
  slot_nm: [2_400_000, 1_000_000],
};

/** The same pad with a round hole, for the half of this that must not change. */
const ROUND_PAD: PadInfo = { ...SLOTTED_PAD, slot_nm: null };

/** `(drill oval 1.0 1.0)` is legal KiCad and means a 1mm drill. */
const SQUARE_OVAL: PadInfo = { ...SLOTTED_PAD, slot_nm: [1_000_000, 1_000_000] };

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
    'J1',
    new Map(),
    'full' as never,
  );
  return calls;
}

/** The shape the renderer laid down for the hole itself, ring excluded. */
function holeShape(
  calls: { method: string; args: unknown[] }[],
): { kind: 'circle'; radius: number } | { kind: 'oblong'; width: number; height: number } {
  // The copper comes first, then the hole in the drill colour, then the
  // plating ring in its own stroke colour. The hole is the slice between.
  const from = calls.findIndex(
    (call) => call.method === 'set:fillStyle' && call.args[0] === '#1A1A1A',
  );
  const rest = calls.slice(from + 1);
  const ring = rest.findIndex((call) => call.method === 'set:strokeStyle');
  const hole = ring < 0 ? rest : rest.slice(0, ring);

  const arc = hole.find((call) => call.method === 'arc');
  if (arc) return { kind: 'circle', radius: arc.args[2] as number };

  const points = hole
    .filter((call) => ['moveTo', 'lineTo'].includes(call.method))
    .map((call) => [call.args[0] as number, call.args[1] as number]);
  const xs = points.map(([x]) => x);
  const ys = points.map(([, y]) => y);
  return { kind: 'oblong', width: Math.max(...xs) - Math.min(...xs), height: Math.max(...ys) - Math.min(...ys) };
}

describe('a slotted hole on screen', () => {
  it('is drawn at its full length, not at its drill', () => {
    // 2.4mm at 20 screen px per millimetre is 48px across; the drill number
    // alone would draw a 20px circle, which is what this used to do.
    const shape = holeShape(draw(SLOTTED_PAD));

    expect(shape.kind, 'a slot is not a circle').toBe('oblong');
    if (shape.kind !== 'oblong') return;
    expect(shape.width, 'the slot is 2.4mm long, which is 48px here').toBeCloseTo(48, 0);
    expect(shape.height, 'and 1.0mm wide, which is 20px').toBeCloseTo(20, 0);
  });

  it('leaves a round hole round', () => {
    const shape = holeShape(draw(ROUND_PAD));

    expect(shape.kind).toBe('circle');
    if (shape.kind !== 'circle') return;
    expect(shape.radius, 'a 1mm hole is 10px of radius here').toBeCloseTo(10, 0);
  });

  it('treats a square oval as the round hole it is', () => {
    // `(drill oval 1.0 1.0)` means a 1mm drill, so nothing here may treat it
    // as a slot of zero length.
    const shape = holeShape(draw(SQUARE_OVAL));

    expect(shape.kind).toBe('circle');
    if (shape.kind !== 'circle') return;
    expect(shape.radius).toBeCloseTo(10, 0);
  });
});
