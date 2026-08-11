import { describe, it, expect } from 'vitest';
import { drawTrace } from '../renderer';
import { createLayerVisibility } from '../layers';
import type { TraceInfo } from '../types';
import type { Viewport } from '../viewport';

/**
 * Colour is how a PCB tool says which side of the board a trace is on. This
 * one coloured every trace by its net instead, and not as a setting: three
 * copies of `const colorByNet = true` with no way to turn it off. So the
 * layer colours `getTraceColor` defines were unreachable for traces, and the
 * screen could not answer the first question anybody asks about a routed
 * board - which layer is this on.
 *
 * Both readings are useful, which is the point: net colours say what is
 * connected to what, layer colours say which side it runs on. The renderer
 * always took a flag; nothing ever passed it false.
 */
function recordingContext() {
  const strokes: string[] = [];
  const target: Record<string, unknown> = {};
  const ctx = new Proxy(target, {
    get(_t, prop: string) {
      if (prop in target) return target[prop];
      return () => {};
    },
    set(_t, prop: string, value) {
      target[prop] = value;
      if (prop === 'strokeStyle') strokes.push(String(value));
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
  return { ctx, strokes };
}

const VIEWPORT: Viewport = {
  centerX: 10_000_000,
  centerY: 10_000_000,
  scale: 0.00002,
  width: 400,
  height: 400,
};

function trace(id: number, layer: string, net: string): TraceInfo {
  return {
    id,
    net_name: net,
    layer,
    width: 254_000,
    locked: false,
    segments: [
      { start_x: 5_000_000, start_y: 10_000_000, end_x: 15_000_000, end_y: 10_000_000 },
    ],
  } as TraceInfo;
}

function coloursOf(t: TraceInfo, colorByNet: boolean): string[] {
  const { ctx, strokes } = recordingContext();
  drawTrace(ctx, VIEWPORT, t, createLayerVisibility(), colorByNet, null, null, null);
  return strokes;
}

describe('trace colour', () => {
  it('says which layer a trace is on when asked to', () => {
    const top = coloursOf(trace(1, 'Top', 'VCC'), false);
    const bottom = coloursOf(trace(2, 'Bottom', 'VCC'), false);

    expect(top.length).toBeGreaterThan(0);
    expect(bottom.length).toBeGreaterThan(0);
    expect(top[0]).not.toBe(bottom[0]);
  });

  it('says which net a trace carries when asked to', () => {
    const vcc = coloursOf(trace(1, 'Top', 'VCC'), true);
    const gnd = coloursOf(trace(2, 'Top', 'GND'), true);

    expect(vcc[0]).not.toBe(gnd[0]);
  });

  it('is one or the other, not both at once', () => {
    // Two traces on different layers carrying the same net: coloured by net
    // they match, coloured by layer they do not. If either reading collapsed
    // into the other, one of these pairs would stop distinguishing anything.
    const byNet = [
      coloursOf(trace(1, 'Top', 'VCC'), true)[0],
      coloursOf(trace(2, 'Bottom', 'VCC'), true)[0],
    ];
    const byLayer = [
      coloursOf(trace(1, 'Top', 'VCC'), false)[0],
      coloursOf(trace(2, 'Bottom', 'VCC'), false)[0],
    ];

    expect(byNet[0]).toBe(byNet[1]);
    expect(byLayer[0]).not.toBe(byLayer[1]);
  });
});
