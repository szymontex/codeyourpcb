import { describe, it, expect } from 'vitest';
import {
  getTraceColor,
  innerLayerColor,
  innerVisibleFromUrlLayers,
  INNER_LAYER_COLORS,
  createLayerVisibility,
  innerLayerIndex,
  innerLayerDepth,
  viaSpanDepths,
} from '../layers';

/**
 * A four-layer board was drawn as though it had no middle: every inner layer
 * came back the same forest green, and only if an outer layer happened to be
 * visible. Two traces that cannot cross looked identical, and turning the
 * outer copper off took the inner copper with it.
 */
describe('inner copper on screen', () => {
  const visible = createLayerVisibility();

  it('gives each inner layer its own shade', () => {
    const first = getTraceColor('Inner1', visible);
    const second = getTraceColor('Inner2', visible);

    expect(first).toBe(INNER_LAYER_COLORS[0]);
    expect(second).toBe(INNER_LAYER_COLORS[1]);
    expect(first).not.toBe(second);
  });

  it('does not hide inner copper when an outer layer is turned off', () => {
    const outerOff = { ...visible, topCopper: false, bottomCopper: false };
    expect(getTraceColor('Inner1', outerOff)).toBe(INNER_LAYER_COLORS[0]);
  });

  it('hides inner copper when the inner layers are turned off', () => {
    const innerOff = { ...visible, innerCopper: false };
    expect(getTraceColor('Inner1', innerOff)).toBeNull();
  });

  it('counts Inner1 as the first inner layer, the way the DSL writes it', () => {
    expect(innerLayerColor('Inner1', visible)).toBe(INNER_LAYER_COLORS[0]);
    expect(innerLayerColor('Inner4', visible)).toBe(INNER_LAYER_COLORS[3]);
  });

  it('says nothing about a layer name it does not recognise', () => {
    expect(innerLayerColor('Middle', visible)).toBeNull();
  });
});

describe('a shared link and the inner layers', () => {
  it('treats silence as visible, so an old link keeps the middle', () => {
    expect(innerVisibleFromUrlLayers(['top', 'bottom', 'ratsnest'])).toBe(true);
  });

  it('hides them only when the link says so', () => {
    expect(innerVisibleFromUrlLayers(['top', 'no-inner'])).toBe(false);
  });
});

describe('where an inner layer sits', () => {
  it('reads Inner1 as the first inner layer and an outer name as none', () => {
    expect(innerLayerIndex('Inner1')).toBe(0);
    expect(innerLayerIndex('Inner2')).toBe(1);
    expect(innerLayerIndex('Top')).toBeNull();
    expect(innerLayerIndex('Bottom')).toBeNull();
    // The engine's zero-based name is gone; if it ever comes back it names no
    // layer rather than silently meaning the first one.
    expect(innerLayerIndex('Inner0')).toBeNull();
  });

  it('spaces two inner layers evenly through a 1.6mm board', () => {
    // Faces at -0.8 and +0.8, so the pair sits a third of the way in from each.
    // `Inner1` is the copper directly beneath `Top`, so it is the one in the
    // upper half. This asserted the opposite until 2026-08-24, which drew a
    // four-layer board with its two inner layers swapped.
    expect(innerLayerDepth(0, 2, 1.6)).toBeCloseTo(0.2667, 3);
    expect(innerLayerDepth(1, 2, 1.6)).toBeCloseTo(-0.2667, 3);
  });

  it('counts down from the top face', () => {
    // Said as an order rather than as two numbers: whatever the thickness or
    // the count, a lower index is nearer the top. Numbers alone let an
    // inversion be corrected into a different inversion.
    for (const [count, thickness] of [
      [2, 1.6],
      [4, 1.6],
      [6, 0.8],
    ] as const) {
      for (let index = 1; index < count; index++) {
        expect(innerLayerDepth(index - 1, count, thickness)).toBeGreaterThan(
          innerLayerDepth(index, count, thickness),
        );
      }
      expect(innerLayerDepth(0, count, thickness)).toBeLessThan(thickness / 2);
      expect(innerLayerDepth(count - 1, count, thickness)).toBeGreaterThan(-thickness / 2);
    }
  });

  it('puts a single inner layer in the middle', () => {
    expect(innerLayerDepth(0, 1, 1.6)).toBeCloseTo(0, 6);
  });
});

describe('how deep a via goes', () => {
  it('takes a through via from face to face', () => {
    const span = viaSpanDepths('Top', 'Bottom', 2, 1.6);
    expect(span.top).toBeCloseTo(0.8, 6);
    expect(span.bottom).toBeCloseTo(-0.8, 6);
  });

  it('stops a blind via at the inner layer it reaches', () => {
    // Top face down to the first inner layer of a four-layer stack: a short
    // hole in the upper third, not one that crosses most of the board. This
    // asserted -0.2667 while its own comment said "down to the first inner
    // layer", so a blind via was drawn as deep as a buried one.
    const span = viaSpanDepths('Top', 'Inner1', 2, 1.6);
    expect(span.top).toBeCloseTo(0.8, 6);
    expect(span.bottom).toBeCloseTo(0.2667, 3);
    expect(span.top - span.bottom).toBeLessThan(1.6 / 2);
  });

  it('drills deeper for Inner2 than for Inner1', () => {
    // The pair that makes the direction a statement rather than a number.
    const shallow = viaSpanDepths('Top', 'Inner1', 2, 1.6);
    const deeper = viaSpanDepths('Top', 'Inner2', 2, 1.6);
    expect(deeper.top - deeper.bottom).toBeGreaterThan(shallow.top - shallow.bottom);
  });

  it('buries a via between two inner layers', () => {
    const span = viaSpanDepths('Inner1', 'Inner2', 2, 1.6);
    expect(span.bottom).toBeCloseTo(-0.2667, 3);
    expect(span.top).toBeCloseTo(0.2667, 3);
  });

  it('treats a via that says nothing as going through', () => {
    const span = viaSpanDepths('Top', 'Bottom', 0, 1.6);
    expect(span.top - span.bottom).toBeCloseTo(1.6, 6);
  });
});
