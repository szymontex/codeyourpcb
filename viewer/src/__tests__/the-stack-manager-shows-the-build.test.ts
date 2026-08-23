import { describe, it, expect } from 'vitest';
import { stackRows, stackSummary } from '../stack-panel';
import type { StackupInfo } from '../types';

/**
 * The stack manager shows the board's own build.
 *
 * Seven pieces of stackup vocabulary landed in the language over two days and
 * the language was the only way to see any of them. These hold what the panel
 * says, without a browser: the two functions that decide it are pure, which is
 * the lesson this project learned from an error list whose three deciding
 * lines lived inside a closure nothing could reach.
 *
 * The DOM assembly is not here. This suite runs in node - `vitest.config.ts`
 * says `environment: 'node'` and nothing in this repository pulls jsdom - so
 * the element tree is held by `viewer/e2e/stack-panel.spec.ts` in a real
 * browser instead, which is a better place to hold a panel anyway.
 */

/** A four-layer stack that states every field the model has. */
function stack(): StackupInfo {
  return {
    layers: [
      {
        kind: 'mask',
        name: 'F.Mask',
        thickness_nm: 20_000,
        sheets_nm: [20_000],
        slot_thickness_nm: 20_000,
        material: '',
        color: 'Matte Black',
      },
      {
        kind: 'copper',
        name: 'F.Cu',
        thickness_nm: 34_998,
        sheets_nm: [34_998],
        slot_thickness_nm: 34_998,
        material: '',
        color: '',
      },
      {
        kind: 'prepreg',
        name: 'dielectric 1',
        thickness_nm: 66_800,
        sheets_nm: [66_800, 66_800],
        slot_thickness_nm: 133_600,
        material: 'FR4',
        color: '',
        dk_x1000: 4_500,
        df_x1000000: 20_000,
      },
      {
        kind: 'copper',
        name: '',
        thickness_nm: 17_499,
        sheets_nm: [17_499],
        slot_thickness_nm: 17_499,
        material: '',
        color: '',
      },
    ],
    finish: 'ENIG',
    edges_plated: true,
    castellated_pads: false,
    edge_connector: 'bevelled',
    impedance_controlled: true,
    drill_pairs: [['Top', 'Inner1']],
    total_thickness_nm: 206_097,
  };
}

describe('what each layer of the stack shows', () => {
  it('names a layer what the design named it', () => {
    expect(stackRows(stack())[0].label).toBe('F.Mask');
  });

  it('falls back to what the layer is when the design named nothing', () => {
    // Most designs name no layer, and a blank row is worse than a plain one.
    expect(stackRows(stack())[3].label).toBe('copper');
  });

  it('shows the whole slot, not its first sheet', () => {
    // A fabricator hits a target thickness with the prepreg they stock, so a
    // panel that showed 0.0668mm would show a thinner board than the one being
    // built.
    const prepreg = stackRows(stack())[2];
    expect(prepreg.thickness).toBe('0.1336mm');
    expect(prepreg.detail).toContain('2 sheets: 0.0668mm + 0.0668mm');
  });

  it('prints the numbers a fabricator reads, in their own units', () => {
    const prepreg = stackRows(stack())[2];
    expect(prepreg.detail).toContain('FR4');
    expect(prepreg.detail).toContain('dk 4.5');
    expect(prepreg.detail).toContain('df 0.02');
  });

  it('carries the colour of a mask', () => {
    expect(stackRows(stack())[0].detail).toContain('Matte Black');
  });

  it('trims a thickness rather than printing six decimals', () => {
    expect(stackRows(stack())[1].thickness).toBe('0.035mm');
  });

  it('sizes each bar by its share of the board', () => {
    const rows = stackRows(stack());
    const total = rows.reduce((sum, row) => sum + row.share, 0);
    expect(total).toBeCloseTo(1, 6);
    // And the copper foil is a sliver next to the dielectric.
    expect(rows[2].share).toBeGreaterThan(rows[1].share);
  });

  it('draws no bar at all when the board states no total', () => {
    // A bar drawn from a guess is a drawing that lies.
    const partial = stack();
    partial.total_thickness_nm = undefined;
    expect(stackRows(partial).every((row) => row.share === 0)).toBe(true);
  });
});

describe('what the fabricator is asked for beyond the layers', () => {
  it('lists every statement the design made', () => {
    expect(stackSummary(stack())).toEqual([
      // Four decimals is 0.1 micrometre, finer than any fab tolerance and
      // shorter than the six the writer uses for a round trip.
      '0.2061mm thick',
      'finish ENIG',
      'impedance controlled',
      'edges plated',
      'bevelled edge connector',
      'drills Top to Inner1',
    ]);
  });

  it('says nothing about what the design did not ask for', () => {
    // Silence is the rest, the way it is in the language: a design that wants
    // no castellated pads does not get a row saying so.
    expect(stackSummary(stack())).not.toContain('castellated pads');
  });

  it('is empty for a stack that states only its layers', () => {
    const bare: StackupInfo = {
      layers: [],
      finish: '',
      edges_plated: false,
      castellated_pads: false,
      edge_connector: '',
      impedance_controlled: false,
      drill_pairs: [],
    };
    expect(stackSummary(bare)).toEqual([]);
  });
});
