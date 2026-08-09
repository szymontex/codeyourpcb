/**
 * A part on the back of the board is drawn as being on the back.
 *
 * `npx vitest run src/__tests__/a-bottom-part-is-drawn-as-one.test.ts`
 *
 * `side bottom` reaches the browser correctly for copper: the engine holds the
 * flipped footprint, so a bottom part's pads arrive with bottom-copper layer
 * bits and mirrored coordinates, and `getPadColor` already refuses to draw
 * them when the bottom copper is hidden.
 *
 * Its ink did not. Silkscreen and the body outline come from a footprint, and
 * a footprint has no side of its own - the part does. The renderer drew both
 * in the top silkscreen colour whatever side the part was on, so a bottom part
 * printed its legend on the top of the board and looked identical to a
 * top-side part whose pads had gone to the wrong layer.
 */
import { describe, it, expect } from 'vitest';

import { componentSilkColor } from '../renderer';
import { LAYER_COLORS } from '../layers';
import { createDefaultRenderConfig } from '../render-config';
import type { ComponentInfo } from '../types';

function part(side?: 'top' | 'bottom'): ComponentInfo {
  return {
    refdes: 'R1',
    value: '10k',
    x_nm: 10_000_000,
    y_nm: 10_000_000,
    rotation_mdeg: 0,
    footprint: '0402',
    pads: [],
    body_width_nm: 1_000_000,
    body_height_nm: 500_000,
    side,
    model_3d: null,
    silk: [],
  };
}

describe('the legend of a part', () => {
  const config = createDefaultRenderConfig();

  it('is printed on the back when the part is on the back', () => {
    expect(componentSilkColor(part('bottom'), config)).toBe(LAYER_COLORS.bottom_silk);
  });

  it('stays on the front for a part on the front', () => {
    expect(componentSilkColor(part('top'), config)).toBe(config.layerColors.silkscreen);
  });

  it('and a part that says nothing is a part on the front', () => {
    // Every design in this repository states no side.
    expect(componentSilkColor(part(undefined), config)).toBe(config.layerColors.silkscreen);
  });

  it('the two colours are actually different', () => {
    // Guard the guard: if the palette ever gave both sides the same ink, all
    // three tests above would pass while the screen told nobody anything.
    expect(LAYER_COLORS.bottom_silk).not.toBe(config.layerColors.silkscreen);
  });
});
