import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { flexRegions } from '../flex-regions';
import type { BoardSnapshot } from '../types';

/**
 * The side view showed a board that cannot fold.
 *
 * The 2D view has drawn the bend since the amber band shipped. The 3D view
 * builds its substrate as one box of the board's thickness, so the one place a
 * flexible region would be most obvious - looking at the board edge-on - said
 * nothing about it.
 */

const SRC = join(__dirname, '..');

function snapshot(zones: unknown[]): BoardSnapshot {
  return { zones } as unknown as BoardSnapshot;
}

/** The ribbon of `examples/rigid-flex.cypcb`: 22mm to 38mm, full height. */
const bend = {
  name: 'bend',
  kind: 'flex',
  layer_mask: 0b11,
  net: '',
  bounds: [22_000_000, 0, 38_000_000, 16_000_000],
};

describe('where the board bends', () => {
  it('comes back in the millimetres the scene is built in', () => {
    const [region] = flexRegions(snapshot([bend]));
    expect(region.name).toBe('bend');
    expect(region.xMm).toBeCloseTo(22, 9);
    expect(region.yMm).toBeCloseTo(0, 9);
    expect(region.widthMm).toBeCloseTo(16, 9);
    expect(region.heightMm).toBeCloseTo(16, 9);
  });

  it('leaves the other kinds alone', () => {
    // A keepout is an absence of copper and a pour is copper: neither is a
    // statement about what the board is made of.
    const others = [
      { ...bend, kind: 'keepout' },
      { ...bend, kind: 'pour' },
    ];
    expect(flexRegions(snapshot(others))).toHaveLength(0);
    expect(flexRegions(snapshot([...others, bend]))).toHaveLength(1);
  });

  it('reads bounds written either way round', () => {
    const backwards = { ...bend, bounds: [38_000_000, 16_000_000, 22_000_000, 0] };
    const [region] = flexRegions(snapshot([backwards]));
    expect(region.xMm).toBeCloseTo(22, 9);
    expect(region.widthMm).toBeCloseTo(16, 9);
  });

  it('drops a region with no area', () => {
    // A box of zero width renders as a plane seen edge-on, which is a line
    // across the board that means nothing.
    const flat = { ...bend, bounds: [22_000_000, 0, 22_000_000, 16_000_000] };
    expect(flexRegions(snapshot([flat]))).toHaveLength(0);
  });

  it('says nothing about a board that carries no zones', () => {
    expect(flexRegions(snapshot([]))).toHaveLength(0);
    expect(flexRegions(null)).toHaveLength(0);
  });
});

describe('the 3D view', () => {
  const source = readFileSync(join(SRC, 'renderer3d.ts'), 'utf8');

  it('draws them', () => {
    // The whole loop, not the import: a module that reads the regions and
    // builds nothing from them is the state this commit found.
    expect(source).toContain('for (const region of flexRegions(snapshot)) {');
    expect(source).toContain("flexMesh.name = region.name ? `flex-region-${region.name}`");
  });

  it('tints rather than thins', () => {
    // A rigid-flex design states one stack for the whole board and says
    // nothing about where a stiffener stops, so a thinner slab through the
    // bend would be a thickness nobody wrote down. The box is the board's own
    // thickness, a hair proud of it so it shows.
    expect(source).toContain('BOARD_THICKNESS_MM * 1.02');
    expect(source).toContain('transparent: true');
  });
});
