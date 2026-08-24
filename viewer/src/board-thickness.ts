/**
 * How thick the board is, and where each layer sits in it.
 *
 * The 3D view drew every board 1.6mm thick. That is the standard, and it is
 * what a board says when it says nothing - but a board can say otherwise now:
 * `stackup { ... }` reaches the snapshot as `total_thickness_nm`, the stack
 * panel prints the real figure, and `examples/rigid-flex.cypcb` is **0.335mm**
 * and drew as a slab five times too thick beside a panel stating the truth.
 *
 * The arithmetic lives here rather than in `renderer3d.ts` so it can be tested
 * without a browser: that module imports three.js and its OrbitControls, which
 * want a DOM, and `vitest.config.ts` runs on node.
 */

import type { BoardSnapshot } from './types';

/** What a board is when it does not say: 1.6mm, the standard FR-4 panel. */
export const DEFAULT_BOARD_THICKNESS_MM = 1.6;

/** What the foil is when the stack does not say: one ounce. */
export const DEFAULT_COPPER_THICKNESS_MM = 0.035;

/** Nanometres to millimetres. */
const NM_TO_MM = 1e-6;

/**
 * The board's own thickness in millimetres, or the standard one.
 *
 * `total_thickness_nm` is absent whenever any layer of the stack stated no
 * thickness - a partial sum would read like a measurement - so absence here
 * means "the board did not say", which is exactly when the default applies.
 * A zero or negative figure is refused for the same reason: it would collapse
 * the scene rather than describe a board.
 */
export function boardThicknessMm(snapshot: BoardSnapshot | null | undefined): number {
  const stated = snapshot?.stackup?.total_thickness_nm;
  if (typeof stated !== 'number' || !Number.isFinite(stated) || stated <= 0) {
    return DEFAULT_BOARD_THICKNESS_MM;
  }
  return stated * NM_TO_MM;
}

/**
 * The foil on one face of the board, in millimetres.
 *
 * The outer copper of the stack: its first `copper` entry for the front, its
 * last for the back. A half-ounce flex and a two-ounce power board are drawn
 * with the foil they are pressed from rather than with one ounce for both -
 * the same figure `TraceCurrentRule` reads through `Stackup::copper_weight_oz`
 * when it decides how wide a trace has to be.
 *
 * Falls back to one ounce when the stack states no copper, or states one with
 * no thickness, for the reason the board's own thickness does.
 */
export function copperThicknessMm(
  snapshot: BoardSnapshot | null | undefined,
  face: 'front' | 'back',
): number {
  const coppers = (snapshot?.stackup?.layers ?? []).filter((layer) => layer.kind === 'copper');
  const at = face === 'front' ? coppers[0] : coppers[coppers.length - 1];
  const stated = at?.thickness_nm;
  if (typeof stated !== 'number' || !Number.isFinite(stated) || stated <= 0) {
    return DEFAULT_COPPER_THICKNESS_MM;
  }
  return stated * NM_TO_MM;
}

/** Where each surface sits on the Z axis, with the board centred on zero. */
export interface ZStack {
  boardTop: number;
  boardBot: number;
  frontCopperBot: number;
  frontCopperTop: number;
  backCopperTop: number;
  backCopperBot: number;
  frontMask: number;
  backMask: number;
  topPad: number;
  bottomPad: number;
}

/**
 * The Z positions for a board of this thickness.
 *
 * KiCad's convention, which this view already followed: the substrate is
 * centred on zero, so the two faces are at plus and minus half the thickness
 * and everything else is measured out from them. Copper and mask keep their
 * own thicknesses - a thin board is thin in its laminate, not in its foil, and
 * the two faces need not carry the same foil.
 */
export function zStack(
  thicknessMm: number,
  frontCopperMm: number,
  backCopperMm: number = frontCopperMm,
): ZStack {
  const boardTop = thicknessMm / 2;
  const boardBot = -thicknessMm / 2;
  const frontCopperTop = boardTop + frontCopperMm;
  const backCopperBot = boardBot - backCopperMm;
  const frontMask = frontCopperTop + 0.001;
  const backMask = backCopperBot - 0.001;
  return {
    boardTop,
    boardBot,
    frontCopperBot: boardTop,
    frontCopperTop,
    backCopperTop: boardBot,
    backCopperBot,
    frontMask,
    backMask,
    topPad: frontCopperTop + 0.003,
    bottomPad: backCopperBot - 0.003,
  };
}
