import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { Renderer3D, silkFace } from '../renderer3d';
import type { ComponentInfo, SilkShape } from '../types';

/**
 * A part's legend is built on the face of the board the part is on.
 *
 * `npx vitest run src/__tests__/a-legend-is-built-on-the-face-its-part-is-on.test.ts`
 *
 * The 3D view chose the face of each legend shape from a `layer` on the shape.
 * The engine never sends one - the board model keeps one legend per footprint
 * and prints it on the part's side, which is what the Gerber does - so every
 * legend of every board arrived without a layer and was built under the board,
 * the legend of a top part included. Measured 2026-10-12 with the engine's own
 * payload: two parts, one on each face, both legends at z = -0.863.
 *
 * The geometry in a snapshot is already the part's own: a bottom part's legend
 * arrives mirrored, as its pads do. `crates/cypcb-render/tests/
 * the_legend_goes_where_the_part_is.rs` holds the same two parts to the
 * Gerber, and the figures below are the ones that test finds in the file.
 */

/** The legend of the part in that test: one line, 1mm to 2mm right of the origin. */
const LINE: SilkShape = { type: 'segment', x1: 1_000_000, y1: 0, x2: 2_000_000, y2: 0, width: 150_000 };

/** The same line as the engine hands it for a part on the bottom: x reversed. */
const MIRRORED: SilkShape = { type: 'segment', x1: -1_000_000, y1: 0, x2: -2_000_000, y2: 0, width: 150_000 };

function part(refdes: string, side: ComponentInfo['side'], xMm: number, silk: SilkShape[]): ComponentInfo {
  return {
    refdes, value: '', x_nm: xMm * 1_000_000, y_nm: 10_000_000, rotation_mdeg: 0, footprint: 'F',
    pads: [], body_width_nm: 0, body_height_nm: 0, side, model_3d: null, silk,
  };
}

/** What `buildSilkscreen` makes of these parts: the two groups it fills. */
function built(components: ComponentInfo[]) {
  const top = new THREE.Group();
  const bottom = new THREE.Group();
  (Renderer3D.prototype as any).buildSilkscreen.call({}, components, top, bottom);
  return { top, bottom };
}

/** Every vertex of a group's meshes, as [x, y, z] in mm. */
function vertices(group: THREE.Group): number[][] {
  const out: number[][] = [];
  for (const mesh of group.children as THREE.Mesh[]) {
    const a = mesh.geometry.getAttribute('position');
    for (let i = 0; i < a.count; i++) out.push([a.getX(i), a.getY(i), a.getZ(i)]);
  }
  return out;
}

/** Quads in a group: a line is one quad of six vertices. */
const quads = (group: THREE.Group) => vertices(group).length / 6;

describe('silkFace', () => {
  it('is the face the part is on', () => {
    expect(silkFace('top')).toBe('top');
    expect(silkFace('bottom')).toBe('bottom');
  });

  it('and a part that says nothing is on the top', () => {
    expect(silkFace(undefined)).toBe('top');
  });
});

describe('the 3D legend of a board', () => {
  it('puts a top part\'s legend on the top face, over the board', () => {
    const { top, bottom } = built([part('R1', 'top', 10, [LINE])]);
    expect(quads(top)).toBe(1);
    expect(quads(bottom)).toBe(0);
    for (const [, , z] of vertices(top)) expect(z).toBeGreaterThan(0);
  });

  it('puts a bottom part\'s legend on the bottom face, under the board', () => {
    const { top, bottom } = built([part('R2', 'bottom', 30, [MIRRORED])]);
    expect(quads(top)).toBe(0);
    expect(quads(bottom)).toBe(1);
    for (const [, , z] of vertices(bottom)) expect(z).toBeLessThan(0);
  });

  it('puts one of each where there is one of each', () => {
    // The board of the Rust test: R1 on the top at 10mm, R2 under it at 30mm.
    const { top, bottom } = built([
      part('R1', 'top', 10, [LINE]),
      part('R2', 'bottom', 30, [MIRRORED]),
    ]);
    expect(quads(top)).toBe(1);
    expect(quads(bottom)).toBe(1);

    // The line the Gerber draws: 11mm to 12mm for R1, 29mm to 28mm for R2.
    const span = (group: THREE.Group) => {
      const xs = vertices(group).map(([x]) => x);
      return [Math.min(...xs), Math.max(...xs)].map((x) => Math.round(x * 1000) / 1000);
    };
    expect(span(top)).toEqual([11, 12]);
    expect(span(bottom)).toEqual([28, 29]);
  });

  it('control: a part that says nothing is built on the top', () => {
    const { top, bottom } = built([part('R3', undefined, 10, [LINE])]);
    expect(quads(top)).toBe(1);
    expect(quads(bottom)).toBe(0);
  });

  it('does not read a layer off the shape: the part decides', () => {
    // The engine refuses the field now; a stale payload that still carried one
    // must not move the legend off the part's face.
    const stale = { ...LINE, layer: 'bottom' } as unknown as SilkShape;
    const { top, bottom } = built([part('R1', 'top', 10, [stale])]);
    expect(quads(top)).toBe(1);
    expect(quads(bottom)).toBe(0);
  });
});
