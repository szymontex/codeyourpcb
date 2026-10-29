import { describe, it, expect } from 'vitest';
import { drawPour, drawKeepout } from '../renderer';
import { colorWithAlpha, getTraceColor, type LayerVisibility } from '../layers';
import type { PourInfo, ZoneInfo } from '../types';
import type { Viewport } from '../viewport';

/**
 * A pour on an inner layer was sent to the fabricator and never to the screen,
 * and a pour that did arrive was coloured as the bottom unless its mask said
 * top. Each fill is on one layer and its mask names it, so it is drawn in that
 * layer's colour and goes when that layer is hidden.
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
const all: LayerVisibility = { topCopper: true, bottomCopper: true, innerCopper: true } as LayerVisibility;

const square: [number, number, number, number][] = [[0, 0, 10_000_000, 10_000_000]];
const onTop: PourInfo = { net: 'GND', layer_mask: 0b0001, rects: square };
const onBottom: PourInfo = { net: 'GND', layer_mask: 0b0010, rects: square };
const onInner1: PourInfo = { net: 'GND', layer_mask: 0b0100, rects: square };
const onInner2: PourInfo = { net: 'GND', layer_mask: 0b1000, rects: square };

function drawn(pour: PourInfo, layers: LayerVisibility) {
  const { ctx, calls } = recordingContext();
  drawPour(ctx, viewport, pour, layers, null);
  const fills = calls.filter((c) => c.method === 'fillRect').length;
  const style = calls.find((c) => c.method === 'set:fillStyle')?.args[0];
  return { fills, style };
}

describe('a pour on an inner layer', () => {
  it('is drawn in the colour of its own layer', () => {
    const inner1 = drawn(onInner1, all);
    expect(inner1.fills).toBe(1);
    const colour = getTraceColor('Inner1', all);
    expect(colour).not.toBeNull();
    expect(inner1.style).toBe(colorWithAlpha(colour as string, 0.35));
    expect(drawn(onInner2, all).style).toBe(colorWithAlpha(getTraceColor('Inner2', all) as string, 0.35));
    expect(inner1.style).not.toBe(drawn(onBottom, all).style);
    expect(inner1.style).not.toBe(drawn(onTop, all).style);
  });

  it('is not drawn when the inner copper is hidden, and the outer ones stay', () => {
    const noInner = { ...all, innerCopper: false } as LayerVisibility;
    expect(drawn(onInner1, noInner).fills).toBe(0);
    expect(drawn(onTop, noInner).fills).toBe(1);
    expect(drawn(onBottom, noInner).fills).toBe(1);
  });

  it('is hidden by its own layer and not by its neighbour', () => {
    const hideFirst = { ...all, hiddenLayers: ['Inner1'] } as LayerVisibility;
    expect(drawn(onInner1, hideFirst).fills).toBe(0);
    expect(drawn(onInner2, hideFirst).fills).toBe(1);
  });

  it('control: top and bottom are drawn as they were', () => {
    expect(drawn(onTop, all).style).toBe(colorWithAlpha(getTraceColor('Top', all) as string, 0.35));
    expect(drawn(onBottom, all).style).toBe(colorWithAlpha(getTraceColor('Bottom', all) as string, 0.35));
    const noTop = { ...all, topCopper: false } as LayerVisibility;
    expect(drawn(onTop, noTop).fills).toBe(0);
  });
});

describe('a keepout on an inner layer', () => {
  const keepout = (mask: number): ZoneInfo => ({
    name: 'k', kind: 'keepout', layer_mask: mask, net: '', bounds: [0, 0, 5_000_000, 5_000_000],
  });
  const strokes = (zone: ZoneInfo, layers: LayerVisibility) => {
    const { ctx, calls } = recordingContext();
    drawKeepout(ctx, viewport, zone, layers);
    return calls.filter((c) => c.method === 'stroke' || c.method === 'strokeRect').length;
  };

  it('is drawn while its layer shows and not when it is hidden', () => {
    expect(strokes(keepout(0b0100), all)).toBeGreaterThan(0);
    expect(strokes(keepout(0b0100), { ...all, innerCopper: false } as LayerVisibility)).toBe(0);
    expect(strokes(keepout(0b0100), { ...all, hiddenLayers: ['Inner1'] } as LayerVisibility)).toBe(0);
  });

  it('control: a top keepout follows the top switch only', () => {
    expect(strokes(keepout(0b0001), all)).toBeGreaterThan(0);
    expect(strokes(keepout(0b0001), { ...all, topCopper: false } as LayerVisibility)).toBe(0);
  });
});
