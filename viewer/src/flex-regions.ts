/**
 * Where the board bends, in millimetres.
 *
 * The 2D view draws these as an amber band. The 3D view drew nothing: its
 * substrate is one box of the board's thickness, so the side view - the one
 * place a flexible region would be most obvious - showed a board that cannot
 * fold.
 *
 * **It is a tint rather than a thinner slab, and that is a decision.** A
 * rigid-flex design in this language states one stack for the whole board:
 * `examples/rigid-flex.cypcb` is coverlay, foil, core, foil, coverlay and a
 * stiffener, 0.335mm of it, and nothing says where the stiffener stops. The
 * bend is thinner on a real board because a layer or two ends before it, and
 * this project has no word for that yet - so drawing a step would be inventing
 * a thickness the design never gave. What the design does say is *this part
 * bends*, and that is what gets drawn.
 *
 * Here rather than in `renderer3d.ts` so it can be tested without a browser:
 * that module imports three.js and its `OrbitControls`, which want a DOM, and
 * `vitest.config.ts` runs on node.
 */

import type { BoardSnapshot } from './types';

/** Nanometres to millimetres. */
const NM_TO_MM = 1e-6;

/** One flexible region, in the millimetre coordinates the 3D scene uses. */
export interface FlexRegionBox {
  /** The name the design gave it, empty when it gave none. */
  name: string;
  /** Its corner, nearest the origin. */
  xMm: number;
  yMm: number;
  widthMm: number;
  heightMm: number;
}

/**
 * Every flexible region the snapshot carries.
 *
 * A region with no area is left out: a zone whose bounds collapse describes
 * nothing to draw, and a box of zero width renders as a plane edge-on.
 */
export function flexRegions(snapshot: BoardSnapshot | null | undefined): FlexRegionBox[] {
  return (snapshot?.zones ?? [])
    .filter((zone) => zone.kind === 'flex')
    .map((zone) => {
      const [x1, y1, x2, y2] = zone.bounds;
      return {
        name: zone.name,
        xMm: Math.min(x1, x2) * NM_TO_MM,
        yMm: Math.min(y1, y2) * NM_TO_MM,
        widthMm: Math.abs(x2 - x1) * NM_TO_MM,
        heightMm: Math.abs(y2 - y1) * NM_TO_MM,
      };
    })
    .filter((box) => box.widthMm > 0 && box.heightMm > 0);
}
