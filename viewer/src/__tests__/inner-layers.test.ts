import { describe, it, expect } from 'vitest';
import { getTraceColor, innerLayerColor, INNER_LAYER_COLORS, createLayerVisibility } from '../layers';

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
