import { describe, it, expect } from 'vitest';
import {
  getTraceColor,
  innerLayerColor,
  innerVisibleFromUrlLayers,
  INNER_LAYER_COLORS,
  createLayerVisibility,
  innerLayerIndex,
  innerLayerDepth,
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
    expect(innerLayerDepth(0, 2, 1.6)).toBeCloseTo(-0.2667, 3);
    expect(innerLayerDepth(1, 2, 1.6)).toBeCloseTo(0.2667, 3);
  });

  it('puts a single inner layer in the middle', () => {
    expect(innerLayerDepth(0, 1, 1.6)).toBeCloseTo(0, 6);
  });
});
