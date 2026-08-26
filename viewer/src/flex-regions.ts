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
 * The stack with its stiffeners taken out, or `null` when there are none.
 *
 * A stiffener is the one layer of a rigid-flex stack that cannot be in a bend:
 * it is bonded on to stop a part of the board flexing, which is the opposite
 * of what the ribbon is for. Its thickness is stated - `examples/rigid-flex.cypcb`
 * says `stiffener 0.2mm material "FR4"` - so taking it off is arithmetic on
 * the design's own figures rather than a thickness invented for the picture.
 *
 * `null` whenever that arithmetic cannot be done from what the board says:
 * no stiffener, no total, or a stiffener with no thickness of its own.
 */
export function flexThicknessMm(snapshot: BoardSnapshot | null | undefined): number | null {
  const layers = snapshot?.stackup?.layers ?? [];
  const stiffeners = layers.filter((layer) => layer.kind === 'stiffener');
  if (stiffeners.length === 0) return null;

  const stated = snapshot?.stackup?.total_thickness_nm;
  if (typeof stated !== 'number' || !Number.isFinite(stated) || stated <= 0) return null;

  let bonded = 0;
  for (const layer of stiffeners) {
    const own = layer.slot_thickness_nm ?? layer.thickness_nm;
    if (typeof own !== 'number' || !Number.isFinite(own) || own <= 0) return null;
    bonded += own;
  }

  const left = (stated - bonded) * NM_TO_MM;
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
