import { describe, it, expect } from 'vitest';
import { dropAt, flexThicknessMm, substrateSlabs } from '../flex-regions';
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

/**
 * The same board with the design saying where its layers stop, which is what
 * `covers` and `outside` are for. The coverlays are over the ribbon and the
 * stiffener is everywhere but - so the bend is the whole stack minus the
 * stiffener, and the rigid ends are what is left.
 */
function stated(options?: { coverlayOverBendOnly?: boolean }) {
  const { coverlayOverBendOnly = false } = options ?? {};
  const snapshot = rigidFlex() as unknown as {
    stackup: { layers: Record<string, unknown>[] };
  };
  for (const layer of snapshot.stackup.layers) {
    if (layer.kind === 'stiffener') {
      layer.coverage_region = 'bend';
      layer.coverage_covers = false;
    }
    if (layer.kind === 'coverlay' && coverlayOverBendOnly) {
      layer.coverage_region = 'bend';
      layer.coverage_covers = true;
    }
  }
  return snapshot as unknown as BoardSnapshot;
}

describe('where a layer stops, the design says', () => {
  it('adds up the layers that are in the bend rather than guessing', () => {
    // The stiffener says `outside bend`, so it is not in the ribbon: the same
    // 135 microns as the inference gives, from the design's own sentence.
    expect(flexThicknessMm(stated())).toBeCloseTo(0.135, 6);
  });

  it('answers differently when the design says something different', () => {
    // Both coverlays say `covers bend` as well, which changes nothing about
    // the ribbon - they are in it - and this is the case the old arithmetic
    // could not tell apart from any other: 135 microns either way, but now
    // because the board says so.
    expect(flexThicknessMm(stated({ coverlayOverBendOnly: true }))).toBeCloseTo(0.135, 6);

    // A coverlay that is over the rigid ends instead is not in the bend, and
    // the figure moves: 135 - 25 - 25 = 85 microns. Nothing in the old
    // arithmetic could produce this number, because nothing could state it.
    const elsewhere = stated() as unknown as {
      stackup: { layers: Record<string, unknown>[] };
    };
    for (const layer of elsewhere.stackup.layers) {
      if (layer.kind === 'coverlay') {
        layer.coverage_region = 'bend';
        layer.coverage_covers = false;
      }
    }
    expect(flexThicknessMm(elsewhere as unknown as BoardSnapshot)).toBeCloseTo(0.085, 6);
  });

  it('draws the step from the stated figure', () => {
    const slabs = substrateSlabs(stated());
    expect(slabs.map((slab) => slab.flex)).toEqual([false, true, false]);
    expect(slabs[1].thicknessMm).toBeCloseTo(0.135, 6);
  });
});

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

  it('brings the faces over the bend down with the laminate', () => {
    const snapshot = rigidFlex();
    const slabs = substrateSlabs(snapshot);

    // 0.335mm of board against 0.135mm of ribbon: each face 0.1mm nearer the
    // middle, because the slabs are centred on Z=0.
    expect(dropAt(slabs, 0.335, 30, 8), 'in the bend').toBeCloseTo(0.1, 6);
    expect(dropAt(slabs, 0.335, 5, 8), 'on the rigid end').toBe(0);
    expect(dropAt(slabs, 0.335, 50, 8), 'on the other rigid end').toBe(0);

    // A board with no step has nothing to bring down.
    const plain = substrateSlabs(rigidFlex({ stiffener: false }));
    expect(dropAt(plain, 0.335, 30, 8)).toBe(0);
  });
});
