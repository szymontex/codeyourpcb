import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import {
  boardThicknessMm,
  zStack,
  DEFAULT_BOARD_THICKNESS_MM,
} from '../board-thickness';
import type { BoardSnapshot } from '../types';

/**
 * A board that says it is 0.335mm thick is not drawn 1.6mm thick.
 *
 * `renderer3d.ts` held `BOARD_THICKNESS_MM = 1.6` and used it in fourteen
 * places - the substrate box, both copper planes, via cylinders, component and
 * label heights. `snapshot.stackup.total_thickness_nm` has carried the board's
 * own figure since the stackup work landed, and the stack panel beside the 3D
 * view printed the true one, so the two surfaces disagreed about the same
 * board. `examples/rigid-flex.cypcb` is 0.334998mm - the figure its exported
 * `.gbrjob` states - and drew as a slab nearly five times too thick.
 */

const SRC = join(__dirname, '..');

function snapshotWith(total_thickness_nm: unknown): BoardSnapshot {
  return { stackup: { total_thickness_nm } } as unknown as BoardSnapshot;
}

describe('the thickness a board states', () => {
  it('is what the board says', () => {
    // The rigid-flex example: coverlay, half-ounce copper, a 50 micron core,
    // half-ounce copper, coverlay and a stiffener.
    expect(boardThicknessMm(snapshotWith(334_998))).toBeCloseTo(0.334998, 9);
  });

  it('is 1.6mm when the board says nothing', () => {
    // `total_thickness_nm` is absent whenever any layer of the stack stated no
    // thickness, so absence means "the board did not say".
    expect(boardThicknessMm(snapshotWith(undefined))).toBe(DEFAULT_BOARD_THICKNESS_MM);
    expect(boardThicknessMm({} as BoardSnapshot)).toBe(DEFAULT_BOARD_THICKNESS_MM);
    expect(boardThicknessMm(null)).toBe(DEFAULT_BOARD_THICKNESS_MM);
  });

  it('refuses a figure that would collapse the scene', () => {
    for (const bad of [0, -1000, Number.NaN, 'thick']) {
      expect(boardThicknessMm(snapshotWith(bad))).toBe(DEFAULT_BOARD_THICKNESS_MM);
    }
  });
});

describe('where the layers sit', () => {
  const COPPER = 0.035;

  it('centres the substrate on zero, as KiCad does', () => {
    const z = zStack(1.6, COPPER);
    expect(z.boardTop).toBeCloseTo(0.8, 9);
    expect(z.boardBot).toBeCloseTo(-0.8, 9);
  });

  it('keeps the foil its own thickness on a thin board', () => {
    // A thin board is thin in its laminate, not in its copper: half-ounce foil
    // is still foil. The front copper sits one foil above the top face
    // wherever that face is.
    const z = zStack(0.334998, COPPER);
    expect(z.boardTop).toBeCloseTo(0.167499, 9);
    expect(z.frontCopperTop - z.boardTop).toBeCloseTo(COPPER, 9);
    expect(z.boardBot - z.backCopperBot).toBeCloseTo(COPPER, 9);
  });

  it('puts mask and pads outside the copper', () => {
    const z = zStack(1.6, COPPER);
    expect(z.frontMask).toBeGreaterThan(z.frontCopperTop);
    expect(z.topPad).toBeGreaterThan(z.frontMask);
    expect(z.backMask).toBeLessThan(z.backCopperBot);
    expect(z.bottomPad).toBeLessThan(z.backMask);
  });
});

describe('the renderer', () => {
  const source = readFileSync(join(SRC, 'renderer3d.ts'), 'utf8');

  it('asks the board before it builds anything', () => {
    // The call has to be in `updateBoard`, which clears and rebuilds the
    // scene: a thickness applied anywhere else would leave geometry from the
    // board before it.
    const start = source.indexOf('updateBoard(snapshot: BoardSnapshot');
    expect(start).toBeGreaterThan(-1);
    const body = source.slice(start, start + 2000);
    expect(body).toContain('applyBoardThickness(snapshot)');
  });

  it('no longer holds 1.6 as the answer', () => {
    expect(source).not.toContain('const BOARD_THICKNESS_MM = 1.6');
  });
});
