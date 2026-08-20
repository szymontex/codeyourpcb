/**
 * Pushing the other layers back, so a board you are working on is readable.
 *
 * The complaint this answers, in the owner's words: layers are a mess. A
 * four-layer board draws every layer at once and the one being routed on
 * looks like the three that are not. Altium spends a key on this and KiCad an
 * opacity slider; here it is one key with three stops.
 */

import { describe, it, expect } from 'vitest';
import {
  createLayerVisibility,
  getTraceColor,
  getPadColor,
  colorWithAlpha,
  nextLayerFocus,
  layerMaskBit,
  copperDrawOrder,
  layerOpacity,
  setLayerOpacity,
  GHOST_GREY,
  INNER_LAYER_COLORS,
  LAYER_PRESETS,
  applyLayerPreset,
  nextLayerPreset,
  isLayerVisible,
  LAYER_COLORS,
  DIMMED_ALPHA,
  type LayerVisibility,
} from '../layers';

/** The preset with this id, so a test names one rather than indexing. */
function byId(id: string) {
  const preset = LAYER_PRESETS.find((entry) => entry.id === id);
  if (!preset) throw new Error(`no preset ${id}`);
  return preset;
}

/** A board with every layer on, drawing on `active`, focused as asked. */
function view(active: string, focus?: LayerVisibility['focus']): LayerVisibility {
  return { ...createLayerVisibility(), activeLayer: active, focus };
}

describe('the layer you are looking at', () => {
  it('leaves every layer alone until asked', () => {
    const v = view('Top');
    expect(getTraceColor('Top', v)).toBe(LAYER_COLORS.top_copper);
    expect(getTraceColor('Bottom', v)).toBe(LAYER_COLORS.bottom_copper);
  });

  it('dims the layers that are not the one being drawn on', () => {
    const v = view('Top', 'dim');
    expect(getTraceColor('Top', v)).toBe(LAYER_COLORS.top_copper);

    const other = getTraceColor('Bottom', v);
    expect(other).not.toBe(LAYER_COLORS.bottom_copper);
    expect(other).toBe(colorWithAlpha(LAYER_COLORS.bottom_copper, DIMMED_ALPHA));
  });

  it('hides them outright on solo, and still draws the active one', () => {
    const v = view('Bottom', 'solo');
    expect(getTraceColor('Bottom', v)).toBe(LAYER_COLORS.bottom_copper);
    expect(getTraceColor('Top', v)).toBeNull();
    expect(getTraceColor('Inner1', v)).toBeNull();
  });

  /** An inner layer is the case the two-bit version of this could not reach. */
  it('keeps an inner layer when that is the one being drawn on', () => {
    const v = view('Inner2', 'solo');
    expect(getTraceColor('Inner2', v)).not.toBeNull();
    expect(getTraceColor('Inner1', v)).toBeNull();
    expect(getTraceColor('Top', v)).toBeNull();
  });

  /**
   * Focus decides how loudly a layer is drawn, never whether a hidden one
   * comes back. The View menu is the only thing that answers that.
   */
  it('does not resurrect a layer the view menu turned off', () => {
    const hidden: LayerVisibility = {
      ...createLayerVisibility(),
      bottomCopper: false,
      activeLayer: 'Bottom',
      focus: 'solo',
    };
    expect(getTraceColor('Bottom', hidden)).toBeNull();
  });

  /**
   * A through-hole pad is on every copper layer, so solo never takes one
   * away. A board of headers would otherwise blank its own canvas.
   */
  it('keeps a through-hole pad whatever layer is active', () => {
    const both = layerMaskBit('Top') | layerMaskBit('Bottom');
    expect(getPadColor(both, view('Top', 'solo'))).not.toBeNull();
    expect(getPadColor(both, view('Bottom', 'solo'))).not.toBeNull();
  });

  it('hides an SMD pad that is not on the active layer', () => {
    const topOnly = layerMaskBit('Top');
    expect(getPadColor(topOnly, view('Top', 'solo'))).not.toBeNull();
    expect(getPadColor(topOnly, view('Bottom', 'solo'))).toBeNull();
  });

  it('walks all, grey, dim, solo and back', () => {
    expect(nextLayerFocus(undefined)).toBe('ghost');
    expect(nextLayerFocus('all')).toBe('ghost');
    expect(nextLayerFocus('ghost')).toBe('dim');
    expect(nextLayerFocus('dim')).toBe('solo');
    expect(nextLayerFocus('solo')).toBe('all');
  });

  /**
   * Grey rather than faint, which is the difference between context and
   * competition. A faint red trace still reads as top copper and pulls the
   * eye; a grey one reads as something underneath. Altium calls this
   * grey-scale mode and it is the answer to "show the other one differently".
   */
  it('draws the other layers in grey, keeping their shape', () => {
    const v = view('Top', 'ghost');
    expect(getTraceColor('Top', v)).toBe(LAYER_COLORS.top_copper);
    expect(getTraceColor('Bottom', v)).toBe(GHOST_GREY);
    expect(getTraceColor('Inner1', v)).toBe(GHOST_GREY);
  });

  /**
   * The stack, then the active layer again on top.
   *
   * The renderer painted bottom, then top, then the inner layers - so on a
   * four-layer board Inner1 covered both outer ones, the opposite of the stack
   * it represents. And nothing gave the active layer priority, so drawing on
   * the bottom meant watching top copper paint over the trace under the
   * cursor.
   */
  it('paints the stack in order and the active layer last', () => {
    const present = ['Top', 'Inner2', 'Bottom', 'Inner1'];

    expect(copperDrawOrder(present, undefined)).toEqual([
      'Bottom', 'Inner1', 'Inner2', 'Top',
    ]);

    // Drawing on the bottom: everything else first, the bottom last.
    expect(copperDrawOrder(present, 'Bottom')).toEqual([
      'Inner1', 'Inner2', 'Top', 'Bottom',
    ]);

    // And an inner layer is not buried by the outer ones any more.
    expect(copperDrawOrder(present, 'Inner1')).toEqual([
      'Bottom', 'Inner2', 'Top', 'Inner1',
    ]);
  });

  /**
   * The per-layer half of the same idea `X` answers in one step.
   *
   * A dense four-layer board wants the layer directly under the one being
   * routed heavier than the one two below it, and a single control cannot say
   * that. KiCad ships the same pairing.
   */
  it('draws a layer at the weight it was given', () => {
    const half = setLayerOpacity(view('Top'), 'Bottom', 0.5);
    expect(getTraceColor('Top', half)).toBe(LAYER_COLORS.top_copper);
    expect(getTraceColor('Bottom', half)).toBe(
      colorWithAlpha(LAYER_COLORS.bottom_copper, 0.5),
    );
  });

  it('leaves a layer nobody weighted at full strength', () => {
    expect(layerOpacity('Bottom', view('Top'))).toBe(1);
    expect(getTraceColor('Bottom', view('Top'))).toBe(LAYER_COLORS.bottom_copper);
  });

  /** Weight and focus answer different questions, so they multiply. */
  it('combines a layer weight with the focus mode', () => {
    const dimmedAndHalved = setLayerOpacity(view('Top', 'dim'), 'Bottom', 0.5);
    expect(getTraceColor('Bottom', dimmedAndHalved)).toBe(
      colorWithAlpha(LAYER_COLORS.bottom_copper, DIMMED_ALPHA * 0.5),
    );
  });

  it('draws nothing at all at zero, and clamps what it is given', () => {
    expect(getTraceColor('Bottom', setLayerOpacity(view('Top'), 'Bottom', 0))).toBeNull();
    expect(layerOpacity('Bottom', setLayerOpacity(view('Top'), 'Bottom', 5))).toBe(1);
    expect(layerOpacity('Bottom', setLayerOpacity(view('Top'), 'Bottom', -2))).toBe(0);
  });

  /**
   * The middle of a four-layer board was the one part nobody could recolour.
   *
   * The outer two have had a preference key each since preferences existed;
   * the inner ones came from a constant in this module, so a person who wanted
   * Inner1 and Inner2 further apart had nowhere to say so - and telling two
   * inner layers apart is exactly where colour earns its keep.
   */
  it('draws an inner layer in the colour the view carries', () => {
    const recoloured: LayerVisibility = {
      ...createLayerVisibility(),
      innerColors: ['#112233', '#445566'],
    };
    expect(getTraceColor('Inner1', recoloured)).toBe('#112233');
    expect(getTraceColor('Inner2', recoloured)).toBe('#445566');
  });

  it('falls back to the shipped palette when the view carries none', () => {
    expect(getTraceColor('Inner1', createLayerVisibility())).toBe(INNER_LAYER_COLORS[0]);
  });

  /** More inner layers than colours wraps, which is what the renderer did. */
  it('wraps a short palette rather than running out of colours', () => {
    const two: LayerVisibility = {
      ...createLayerVisibility(),
      innerColors: ['#112233', '#445566'],
    };
    expect(getTraceColor('Inner3', two)).toBe('#112233');
  });

  /**
   * A named set of what is shown, switched to in one move.
   *
   * Looking at the front, checking the back, reading the copper with the
   * legend out of the way - a person does this many times an hour, and doing
   * it by hand means six clicks each way.
   */
  it('shows only the front, and only the back', () => {
    const stack = ['Top', 'Inner1', 'Inner2', 'Bottom'];
    const front = applyLayerPreset(view('Top'), byId('front'), stack);

    expect(isLayerVisible('Top', front)).toBe(true);
    expect(isLayerVisible('Bottom', front)).toBe(false);
    expect(isLayerVisible('Inner1', front)).toBe(false);
    expect(isLayerVisible('Silkscreen', front)).toBe(true);

    const back = applyLayerPreset(view('Top'), byId('back'), stack);
    expect(isLayerVisible('Bottom', back)).toBe(true);
    expect(isLayerVisible('Top', back)).toBe(false);
  });

  it('keeps every copper layer and drops the rest for copper only', () => {
    const stack = ['Top', 'Inner1', 'Bottom'];
    const copper = applyLayerPreset(view('Top'), byId('copper'), stack);

    for (const name of stack) expect(isLayerVisible(name, copper)).toBe(true);
    expect(isLayerVisible('Silkscreen', copper)).toBe(false);
    expect(isLayerVisible('SolderMask', copper)).toBe(false);
  });

  /**
   * The active layer belongs to the person, not to the view. Changing how you
   * are looking at a board must not move the layer you are drawing on.
   */
  it('does not move the layer being drawn on', () => {
    const before = view('Inner1');
    const after = applyLayerPreset(before, byId('back'), ['Top', 'Inner1', 'Bottom']);
    expect(after.activeLayer).toBe('Inner1');
  });

  /** A preset that inherits weights does something different every time. */
  it('clears the per-layer weights it did not set', () => {
    const weighted = setLayerOpacity(view('Top'), 'Bottom', 0.2);
    const after = applyLayerPreset(weighted, byId('all'), ['Top', 'Bottom']);
    expect(layerOpacity('Bottom', after)).toBe(1);
  });

  it('walks the presets and wraps', () => {
    expect(nextLayerPreset(undefined).id).toBe('front');
    expect(nextLayerPreset('all').id).toBe('front');
    expect(nextLayerPreset('copper').id).toBe('all');
  });

  /** A layer that is not on the board cannot be promoted to the front. */
  it('ignores an active layer the board does not have', () => {
    expect(copperDrawOrder(['Top', 'Bottom'], 'Inner3')).toEqual(['Bottom', 'Top']);
  });

  /**
   * `colorWithAlpha` returned a hex colour untouched and said so in a comment,
   * which made every dimmed layer identical to an undimmed one. The palette is
   * hex, so that branch was the only one that mattered.
   */
  it('actually makes a hex colour transparent', () => {
    expect(colorWithAlpha('#C41E1E', 0.16)).toBe('rgba(196, 30, 30, 0.16)');
    expect(colorWithAlpha('#abc', 0.5)).toBe('rgba(170, 187, 204, 0.5)');
  });
});
