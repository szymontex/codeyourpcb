/**
 * Where the board bends, in millimetres.
 *
 * The 2D view draws these as an amber band. The 3D view drew nothing: its
 * substrate is one box of the board's thickness, so the side view - the one
 * place a flexible region would be most obvious - showed a board that cannot
 * fold.
 *
 * **The step is drawn from the design's own figures.** A stackup layer states
 * where it stops - `stiffener 0.2mm outside bend`, `coverlay 0.025mm covers
 * bend` - so the thickness in the ribbon is the sum of the layers that are
 * there rather than a number invented for the picture. A board that says
 * nothing about where its layers stop is still drawn as one slab with the
 * amber tint, which is what this view did before the language could say it.
 *
 * Here rather than in `renderer3d.ts` so it can be tested without a browser:
 * that module imports three.js and its `OrbitControls`, which want a DOM, and
 * `vitest.config.ts` runs on node.
 */

import { boardThicknessMm } from './board-thickness';
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

/** One box of laminate the board is drawn from. */
export interface SubstrateSlab {
  xMm: number;
  yMm: number;
  widthMm: number;
  heightMm: number;
  thicknessMm: number;
  /** True for the part that bends. */
  flex: boolean;
}

/** Millimetre slop, so a band flush with the board edge counts as flush. */
const EPS_MM = 1e-6;

/**
 * How thick the board is where it bends, or `null` when the design does not say.
 *
 * Two ways to know, and the design's own sentence comes first.
 *
 * A stackup layer can state where it stops - `stiffener 0.2mm outside bend`,
 * `coverlay 0.025mm covers bend` - so the bend is the sum of the layers that
 * are there. A layer that says nothing is pressed across the whole panel and
 * counts everywhere; a layer bounded by some other area does not count here.
 *
 * When no layer says anything, the older arithmetic still answers: a stiffener
 * is the one layer that cannot be in a bend, because it is bonded on to stop a
 * part of the board flexing, so the bend is the stack minus every stiffener.
 * That inference is why the clause exists - it is true of a stiffener and of
 * nothing else, and a coverlay ending before the rigid part was unsayable.
 *
 * `null` whenever the arithmetic cannot be done from what the board says: no
 * figure for the whole stack, or a layer that is in the bend and states no
 * thickness. A thickness invented for the picture is worse than a flat board.
 */
export function flexThicknessMm(snapshot: BoardSnapshot | null | undefined): number | null {
  const layers = snapshot?.stackup?.layers ?? [];
  if (layers.length === 0) return null;

  const bends = new Set(
    (snapshot?.zones ?? [])
      .filter((zone) => zone.kind === 'flex' && zone.name !== '')
      .map((zone) => zone.name),
  );
  const stated = layers.filter((layer) => (layer.coverage_region ?? '') !== '');

  if (stated.length > 0) {
    let inTheBend = 0;
    for (const layer of layers) {
      const region = layer.coverage_region ?? '';
      // A layer bounded by an area that is not a bend is somewhere else on the
      // panel: `covers rigid_left` is not over the ribbon, and `outside
      // rigid_left` is.
      const here = region === '' ? true : bends.has(region) === layer.coverage_covers;
      if (!here) continue;
      const own = layer.slot_thickness_nm ?? layer.thickness_nm;
      if (typeof own !== 'number' || !Number.isFinite(own) || own <= 0) return null;
      inTheBend += own;
    }
    const mm = inTheBend * NM_TO_MM;
    return mm > 0 ? mm : null;
  }

  const stiffeners = layers.filter((layer) => layer.kind === 'stiffener');
  if (stiffeners.length === 0) return null;

  const whole = snapshot?.stackup?.total_thickness_nm;
  if (typeof whole !== 'number' || !Number.isFinite(whole) || whole <= 0) return null;

  let bonded = 0;
  for (const layer of stiffeners) {
    const own = layer.slot_thickness_nm ?? layer.thickness_nm;
    if (typeof own !== 'number' || !Number.isFinite(own) || own <= 0) return null;
    bonded += own;
  }

  const left = (whole - bonded) * NM_TO_MM;
  return left > 0 ? left : null;
}

/** Bands of one axis, overlaps merged, in order. */
function merged(bands: { start: number; end: number }[]): { start: number; end: number }[] {
  const sorted = bands.filter((band) => band.end > band.start).sort((a, b) => a.start - b.start);
  const out: { start: number; end: number }[] = [];
  for (const band of sorted) {
    const last = out[out.length - 1];
    if (last && band.start <= last.end + EPS_MM) {
      last.end = Math.max(last.end, band.end);
    } else {
      out.push({ ...band });
    }
  }
  return out;
}

/**
 * The board as boxes: one for a plain board, and a thinner one where it bends.
 *
 * The 3D view drew a single slab of the whole board's thickness, so the side
 * view - the one place a bend is obvious - showed a board that cannot fold.
 *
 * A step is only drawn where the design gives the two figures it needs: a
 * stiffener with a thickness, and a bend that crosses the board. A ribbon that
 * stops short of both edges would leave laminate on either side of it that
 * this shape cannot describe - the board would be thinner in a rectangle in
 * the middle of a sheet - so the whole board is drawn at one thickness and the
 * tint says where it bends, which is what the view did before.
 */
export function substrateSlabs(snapshot: BoardSnapshot | null | undefined): SubstrateSlab[] {
  const board = snapshot?.board;
  if (!board) return [];

  const widthMm = board.width_nm * NM_TO_MM;
  const heightMm = board.height_nm * NM_TO_MM;
  const thicknessMm = boardThicknessMm(snapshot);
  const whole: SubstrateSlab[] = [
    { xMm: 0, yMm: 0, widthMm, heightMm, thicknessMm, flex: false },
  ];
  if (widthMm <= 0 || heightMm <= 0) return whole;

  const bendThickness = flexThicknessMm(snapshot);
  if (bendThickness === null) return whole;

  const regions = flexRegions(snapshot);

  // Across the board one way or the other. A band has to reach both edges of
  // the axis it does not run along, or there is laminate beside it.
  const acrossY = merged(
    regions
      .filter((r) => r.yMm <= EPS_MM && r.yMm + r.heightMm >= heightMm - EPS_MM)
      .map((r) => ({ start: Math.max(0, r.xMm), end: Math.min(widthMm, r.xMm + r.widthMm) })),
  );
  const acrossX = merged(
    regions
      .filter((r) => r.xMm <= EPS_MM && r.xMm + r.widthMm >= widthMm - EPS_MM)
      .map((r) => ({ start: Math.max(0, r.yMm), end: Math.min(heightMm, r.yMm + r.heightMm) })),
  );

  const bands = acrossY.length > 0 ? acrossY : acrossX;
  if (bands.length === 0) return whole;
  const alongX = acrossY.length > 0;
  const span = alongX ? widthMm : heightMm;

  const slabs: SubstrateSlab[] = [];
  const push = (start: number, end: number, flex: boolean) => {
    if (end - start <= EPS_MM) return;
    slabs.push(
      alongX
        ? {
            xMm: start,
            yMm: 0,
            widthMm: end - start,
            heightMm,
            thicknessMm: flex ? bendThickness : thicknessMm,
            flex,
          }
        : {
            xMm: 0,
            yMm: start,
            widthMm,
            heightMm: end - start,
            thicknessMm: flex ? bendThickness : thicknessMm,
            flex,
          },
    );
  };

  let cursor = 0;
  for (const band of bands) {
    push(cursor, band.start, false);
    push(Math.max(0, band.start), Math.min(span, band.end), true);
    cursor = Math.min(span, band.end);
  }
  push(cursor, span, false);

  return slabs.length > 0 ? slabs : whole;
}

/**
 * How far a face drops at a point, in millimetres.
 *
 * The slabs are centred on Z=0, so a bend that is 0.2mm thinner puts both of
 * its faces 0.1mm nearer the middle. Copper, mask and silkscreen over the
 * ribbon have to come with it or they float where the rigid surface used to
 * be - which is what the view did when the substrate learned about the bend
 * and nothing else did.
 *
 * Zero outside a bend, and zero on a board with no step to make.
 */
export function dropAt(
  slabs: SubstrateSlab[],
  boardThicknessMm: number,
  xMm: number,
  yMm: number,
): number {
  for (const slab of slabs) {
    if (!slab.flex) continue;
    const inside =
      xMm >= slab.xMm - EPS_MM &&
      xMm <= slab.xMm + slab.widthMm + EPS_MM &&
      yMm >= slab.yMm - EPS_MM &&
      yMm <= slab.yMm + slab.heightMm + EPS_MM;
    if (inside) {
      const drop = (boardThicknessMm - slab.thicknessMm) / 2;
      return drop > 0 ? drop : 0;
    }
  }
  return 0;
}
