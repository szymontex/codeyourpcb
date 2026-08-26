import { describe, it, expect } from 'vitest';
import { flexThicknessMm, substrateSlabs } from '../flex-regions';
import type { BoardSnapshot } from '../types';

/**
 * The 3D view drew one slab of the whole board's thickness, so a rigid-flex
 * board looked like a board that cannot fold - and the side view is the one
 * place a bend would be obvious.
 *
 * A step needs two figures, and the design has to give both: how thick the
 * bend is, which is the stack minus the stiffener bonded on to stop it
 * flexing, and where the bend crosses the board. Without either, the whole
 * board is one slab and the amber tint says where it bends, which is what the
 * view did before.
 */

/** `examples/rigid-flex.cypcb` in the shape the viewer receives it. */
function rigidFlex(options?: { stiffener?: boolean; bounds?: [number, number, number, number] }) {
  const { stiffener = true, bounds = [22_000_000, 0, 38_000_000, 16_000_000] } = options ?? {};
  const layers = [
    { kind: 'coverlay', name: 'cover top', thickness_nm: 25_000, sheets_nm: [25_000], material: 'PI', color: '', slot_thickness_nm: 25_000 },
    { kind: 'copper', name: 'F.Cu', thickness_nm: 17_500, sheets_nm: [17_500], material: '', color: '', slot_thickness_nm: 17_500 },
    { kind: 'core', name: 'flex core', thickness_nm: 50_000, sheets_nm: [50_000], material: 'PI', color: '', slot_thickness_nm: 50_000 },
    { kind: 'copper', name: 'B.Cu', thickness_nm: 17_500, sheets_nm: [17_500], material: '', color: '', slot_thickness_nm: 17_500 },
    { kind: 'coverlay', name: 'cover bottom', thickness_nm: 25_000, sheets_nm: [25_000], material: 'PI', color: '', slot_thickness_nm: 25_000 },
  ];
  if (stiffener) {
    layers.push({
      kind: 'stiffener',
      name: 'stiffener',
      thickness_nm: 200_000,
      sheets_nm: [200_000],
      material: 'FR4',
      color: '',
      slot_thickness_nm: 200_000,
    });
  }
  const total = layers.reduce((sum, layer) => sum + (layer.slot_thickness_nm ?? 0), 0);
  return {
    board: { width_nm: 60_000_000, height_nm: 16_000_000 },
    stackup: { layers, total_thickness_nm: total },
    zones: [{ kind: 'flex', name: 'bend', bounds, layers: [], net: '' }],
  } as unknown as BoardSnapshot;
}

describe('the bend is drawn thinner', () => {
  it('takes the stiffener off the stack, and nothing else', () => {
    // 25 + 17.5 + 50 + 17.5 + 25 = 135 microns of flex, 200 of stiffener.
    expect(flexThicknessMm(rigidFlex())).toBeCloseTo(0.135, 6);
    expect(flexThicknessMm(rigidFlex({ stiffener: false }))).toBeNull();
  });

  it('splits the board where a bend crosses it', () => {
    const slabs = substrateSlabs(rigidFlex());
    expect(slabs.map((slab) => [slab.xMm, slab.widthMm, slab.flex])).toEqual([
      [0, 22, false],
      [22, 16, true],
      [38, 22, false],
    ]);
    expect(slabs[1].thicknessMm).toBeCloseTo(0.135, 6);
    expect(slabs[0].thicknessMm).toBeCloseTo(0.335, 6);
    // Every slab is the full height of the board: the ribbon runs across it.
    expect(slabs.every((slab) => slab.heightMm === 16 && slab.yMm === 0)).toBe(true);
  });

  it('leaves the board whole when the design has not given both figures', () => {
    // No stiffener: nothing states how thick the bend is.
    expect(substrateSlabs(rigidFlex({ stiffener: false }))).toHaveLength(1);

    // A ribbon that stops short of the edges would leave laminate beside it
    // that a stack of boxes cannot describe.
    const island = substrateSlabs(
      rigidFlex({ bounds: [22_000_000, 4_000_000, 38_000_000, 12_000_000] }),
    );
    expect(island).toHaveLength(1);
    expect(island[0].flex).toBe(false);
  });

  it('draws the step the other way round too', () => {
    // A bend across the short axis: the board splits bottom, bend, top.
    const slabs = substrateSlabs(
      rigidFlex({ bounds: [0, 6_000_000, 60_000_000, 10_000_000] }),
    );
    expect(slabs.map((slab) => [slab.yMm, slab.heightMm, slab.flex])).toEqual([
      [0, 6, false],
      [6, 4, true],
      [10, 6, false],
    ]);
  });
});
