import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  createRoutingState,
  startRoute,
  flipLayer,
  setActiveLayer,
  layerForPad,
  LAYER_BIT,
  type PadHit,
} from '../routing';
import type { ComponentInfo, PadInfo } from '../types';

/**
 * Reported by the owner: "nie mamy zadnego warstw pickera i nie wiemy na jakiej
 * warstwie rysujemy sciezki."
 *
 * Two things were true at once. The layer was never shown - a console log was
 * the only place it appeared - and it was never chosen: `startRoute` read it
 * off the start pad with `(layer_mask & 0x02) ? 'Bottom' : 'Top'`, which tests
 * the bottom bit first. A through-hole pad carries `TopCopper | BottomCopper`
 * (0x03 in `cypcb-world`), so that expression answers `Bottom` for every one
 * of them, and every route from a through-hole pad started on the back of the
 * board with nothing on screen saying so.
 */

const TOP_ONLY = LAYER_BIT.TOP;
const BOTTOM_ONLY = LAYER_BIT.BOTTOM;
const THROUGH_HOLE = LAYER_BIT.TOP | LAYER_BIT.BOTTOM;

function makePad(number: string, layer_mask: number, drill_nm: number | null): PadInfo {
  return {
    number,
    x_nm: 1_000_000,
    y_nm: 1_000_000,
    width_nm: 800_000,
    height_nm: 800_000,
    shape: 'circle',
    layer_mask,
    drill_nm,
  } as PadInfo;
}

function makeHit(pad: PadInfo): PadHit {
  const component: ComponentInfo = {
    refdes: 'J1',
    value: 'HEADER',
    x_nm: 0,
    y_nm: 0,
    rotation_mdeg: 0,
    footprint: 'PinHeader_1x02',
    pads: [pad],
    body_width_nm: 2_540_000,
    body_height_nm: 2_540_000,
  } as ComponentInfo;
  return {
    component,
    pad,
    netName: 'VCC',
    worldX: pad.x_nm,
    worldY: pad.y_nm,
  } as PadHit;
}

describe('the layer a route starts on', () => {
  it('is the pad layer when the pad has copper on one side only', () => {
    // An SMD pad settles it. There is nowhere else to attach, so the active
    // layer does not get a say - in either direction.
    expect(layerForPad({ layer_mask: TOP_ONLY }, 'Bottom')).toBe('Top');
    expect(layerForPad({ layer_mask: BOTTOM_ONLY }, 'Top')).toBe('Bottom');
  });

  it('is the active layer when the pad has copper on both sides', () => {
    // This is the case the old expression got wrong. `0x03 & 0x02` is truthy,
    // so it answered 'Bottom' whatever the user wanted.
    expect(layerForPad({ layer_mask: THROUGH_HOLE }, 'Top')).toBe('Top');
    expect(layerForPad({ layer_mask: THROUGH_HOLE }, 'Bottom')).toBe('Bottom');
  });

  it('starts a through-hole route on Top when Top is active', () => {
    const state = setActiveLayer(createRoutingState(), 'Top');
    const routed = startRoute(state, makeHit(makePad('1', THROUGH_HOLE, 600_000)));
    expect(routed.mode).toBe('routing');
    // The assertion that fails on the old code: it returned 'Bottom' here.
    expect(routed.currentLayer).toBe('Top');
  });

  it('starts a through-hole route on Bottom when Bottom is active', () => {
    const state = setActiveLayer(createRoutingState(), 'Bottom');
    const routed = startRoute(state, makeHit(makePad('1', THROUGH_HOLE, 600_000)));
    expect(routed.currentLayer).toBe('Bottom');
  });

  it('lets a bottom SMD pad override a Top active layer', () => {
    const state = setActiveLayer(createRoutingState(), 'Top');
    const routed = startRoute(state, makeHit(makePad('1', BOTTOM_ONLY, null)));
    expect(routed.currentLayer).toBe('Bottom');
  });
});

describe('the active layer is editor state, not route state', () => {
  it('flips while idle, so the next trace can be aimed before it is drawn', () => {
    // `flipLayer` used to return the state untouched unless a route was in
    // progress, which is what made the layer unpickable.
    const idle = createRoutingState();
    expect(idle.mode).toBe('idle');
    expect(flipLayer(idle).currentLayer).toBe('Bottom');
    expect(flipLayer(flipLayer(idle)).currentLayer).toBe('Top');
  });

  it('refuses a layer name a board cannot carry', () => {
    // `currentLayer` is written straight into every trace this editor makes,
    // so an unknown name here becomes an unparseable file the user has saved.
    const state = createRoutingState();
    expect(setActiveLayer(state, 'Inner1').currentLayer).toBe('Top');
    expect(setActiveLayer(state, '').currentLayer).toBe('Top');
  });

  it('returns the same object when nothing changes, so no redraw is triggered', () => {
    const state = createRoutingState();
    expect(setActiveLayer(state, 'Top')).toBe(state);
  });
});

describe('the picker exists where a user can see it', () => {
  const root = resolve(__dirname, '../../..');
  const html = readFileSync(resolve(root, 'viewer/index.html'), 'utf8');
  const main = readFileSync(resolve(root, 'viewer/src/main.ts'), 'utf8');

  it('is in the toolbar, not behind a menu', () => {
    // The View dropdown already had layer checkboxes and they answer a
    // different question - what is drawn, not what is drawn ON. The point of
    // this control is that the answer is readable without opening anything.
    expect(html).toContain('id="layer-picker"');
    expect(html).toContain('id="layer-pick-top"');
    expect(html).toContain('id="layer-pick-bottom"');
    const pickerAt = html.indexOf('id="layer-picker"');
    const dropdownAt = html.indexOf('id="view-menu-dropdown"');
    expect(pickerAt).toBeGreaterThan(-1);
    expect(dropdownAt).toBeGreaterThan(-1);
    // The picker markup must not be nested inside the dropdown.
    const dropdownEnd = html.indexOf('</div>', dropdownAt);
    expect(pickerAt > dropdownAt && pickerAt < dropdownEnd).toBe(false);
  });

  it('states which of the two is active, for a screen reader as well as an eye', () => {
    expect(html).toMatch(/id="layer-pick-top"[\s\S]{0,120}aria-pressed/);
    expect(html).toMatch(/id="layer-pick-bottom"[\s\S]{0,120}aria-pressed/);
    expect(main).toContain("setAttribute('aria-pressed'");
  });

  it('follows a layer flip made with the keyboard', () => {
    // F during routing goes through onRoutingChange. Without this the picker
    // says one thing while the copper goes somewhere else, which is worse than
    // having no picker.
    expect(main).toMatch(/layerChanged[\s\S]{0,200}syncLayerPicker\(\)/);
  });
});
