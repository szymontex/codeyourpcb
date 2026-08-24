import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import {
  boardThicknessMm,
  copperThicknessMm,
  innerCopperDepthsMm,
  zStack,
  DEFAULT_BOARD_THICKNESS_MM,
  DEFAULT_COPPER_THICKNESS_MM,
} from '../board-thickness';
import { innerLayerDepth, viaSpanDepths } from '../layers';
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

  it('gives the stack a foil for each face', () => {
    // Two arguments, not one: passing a single foil would draw a board with
    // two ounces on the back as though it had one.
    expect(source).toContain("copperThicknessMm(snapshot, 'front')");
    expect(source).toContain("copperThicknessMm(snapshot, 'back')");
  });

  it('hands the stack positions to the traces and the vias alike', () => {
    // A barrel placed by the stack and a trace placed by an even spread would
    // be a via ending beside the copper it lands on.
    expect(source).toContain('innerCopperDepthsMm(snapshot, BOARD_THICKNESS_MM)');
    expect(source).toContain('INNER_DEPTHS?.[innerIndex]');
    expect(source).toContain('INNER_DEPTHS,');
  });

  it('no longer holds 1.6 as the answer', () => {
    expect(source).not.toContain('const BOARD_THICKNESS_MM = 1.6');
  });
});

describe('the foil on each face', () => {
  /** A stack, as the snapshot carries it: outer copper first and last. */
  function stack(layers: Array<{ kind: string; thickness_nm?: number }>): BoardSnapshot {
    return { stackup: { layers } } as unknown as BoardSnapshot;
  }

  it('is the outer copper of the stack, per face', () => {
    // Half an ounce on the front, two ounces on the back - the same board a
    // fabricator would press with different foils on each side. One ounce is
    // 34,998 nm, so half is 17,499 and two are 69,996.
    const board = stack([
      { kind: 'copper', thickness_nm: 17_499 },
      { kind: 'core', thickness_nm: 1_500_000 },
      { kind: 'copper', thickness_nm: 69_996 },
    ]);

    expect(copperThicknessMm(board, 'front')).toBeCloseTo(0.017499, 9);
    expect(copperThicknessMm(board, 'back')).toBeCloseTo(0.069996, 9);
  });

  it('skips whatever is not copper', () => {
    // A rigid-flex stack opens with a coverlay, so the first entry is not the
    // foil and taking it would draw the copper as the film over it.
    const board = stack([
      { kind: 'coverlay', thickness_nm: 25_000 },
      { kind: 'copper', thickness_nm: 17_499 },
      { kind: 'core', thickness_nm: 50_000 },
      { kind: 'copper', thickness_nm: 17_499 },
      { kind: 'coverlay', thickness_nm: 25_000 },
      { kind: 'stiffener', thickness_nm: 200_000 },
    ]);

    expect(copperThicknessMm(board, 'front')).toBeCloseTo(0.017499, 9);
    expect(copperThicknessMm(board, 'back')).toBeCloseTo(0.017499, 9);
  });

  it('is one ounce when the stack does not say', () => {
    expect(copperThicknessMm(stack([]), 'front')).toBe(DEFAULT_COPPER_THICKNESS_MM);
    expect(copperThicknessMm(stack([{ kind: 'copper' }]), 'back')).toBe(
      DEFAULT_COPPER_THICKNESS_MM,
    );
    expect(copperThicknessMm(null, 'front')).toBe(DEFAULT_COPPER_THICKNESS_MM);
  });

  it('places the two faces with their own foil', () => {
    const z = zStack(1.6, 0.017499, 0.069996);
    expect(z.frontCopperTop - z.boardTop).toBeCloseTo(0.017499, 9);
    expect(z.boardBot - z.backCopperBot).toBeCloseTo(0.069996, 9);
  });

  it('reads the back as the front when only one is given', () => {
    const z = zStack(1.6, 0.035);
    expect(z.frontCopperTop - z.boardTop).toBeCloseTo(z.boardBot - z.backCopperBot, 12);
  });
});

describe('where the inner copper sits', () => {
  /**
   * A four-layer build nobody would call unusual: thin prepreg under each
   * outer foil and a thick core in the middle. 0.035 + 0.1 + 0.035 + 1.2 +
   * 0.035 + 0.1 + 0.035 = 1.54mm.
   */
  const FOUR_LAYER = {
    stackup: {
      total_thickness_nm: 1_540_000,
      layers: [
        { kind: 'copper', thickness_nm: 35_000 },
        { kind: 'prepreg', thickness_nm: 100_000 },
        { kind: 'copper', thickness_nm: 35_000 },
        { kind: 'core', thickness_nm: 1_200_000 },
        { kind: 'copper', thickness_nm: 35_000 },
        { kind: 'prepreg', thickness_nm: 100_000 },
        { kind: 'copper', thickness_nm: 35_000 },
      ],
    },
  } as unknown as BoardSnapshot;

  it('follows the stack rather than an even spread', () => {
    const thickness = boardThicknessMm(FOUR_LAYER);
    expect(thickness).toBeCloseTo(1.54, 9);

    const depths = innerCopperDepthsMm(FOUR_LAYER, thickness);
    expect(depths).not.toBeNull();
    // The centre of each inner foil, measured down from the top of the stack:
    // 0.035 + 0.1 + 0.0175 = 0.1525 in, so 0.77 - 0.1525.
    expect(depths![0]).toBeCloseTo(0.6175, 6);
    expect(depths![1]).toBeCloseTo(-0.6175, 6);

    // And that is the point: this build puts its inner copper nearer the
    // faces than equal steps would.
    const even = innerLayerDepth(0, 2, thickness);
    expect(depths![0]).toBeGreaterThan(even);
  });

  it('counts the same direction as the even spread it replaces', () => {
    const depths = innerCopperDepthsMm(FOUR_LAYER, boardThicknessMm(FOUR_LAYER))!;
    expect(depths[0]).toBeGreaterThan(depths[1]);
  });

  it('says nothing rather than something partial', () => {
    const stack = (layers: unknown[]) =>
      ({ stackup: { layers } }) as unknown as BoardSnapshot;

    // A two-layer board has no inner copper to place.
    expect(
      innerCopperDepthsMm(
        stack([
          { kind: 'copper', thickness_nm: 35_000 },
          { kind: 'core', thickness_nm: 1_500_000 },
          { kind: 'copper', thickness_nm: 35_000 },
        ]),
        1.57,
      ),
    ).toBeNull();

    // A hole in the running sum misplaces everything after it, so the even
    // spread stands in rather than a partial answer.
    expect(
      innerCopperDepthsMm(
        stack([
          { kind: 'copper', thickness_nm: 35_000 },
          { kind: 'prepreg' },
          { kind: 'copper', thickness_nm: 35_000 },
          { kind: 'core', thickness_nm: 1_200_000 },
          { kind: 'copper', thickness_nm: 35_000 },
        ]),
        1.54,
      ),
    ).toBeNull();

    expect(innerCopperDepthsMm(null, 1.6)).toBeNull();
  });

  it('lands a blind via on the copper the stack states', () => {
    const thickness = boardThicknessMm(FOUR_LAYER);
    const depths = innerCopperDepthsMm(FOUR_LAYER, thickness);

    const stated = viaSpanDepths('Top', 'Inner1', 2, thickness, depths);
    expect(stated.top).toBeCloseTo(thickness / 2, 9);
    expect(stated.bottom).toBeCloseTo(0.6175, 6);

    // Without the stack it falls back to the even spread, which on this build
    // would end the barrel in laminate a tenth of a millimetre short.
    const guessed = viaSpanDepths('Top', 'Inner1', 2, thickness);
    expect(guessed.bottom).toBeLessThan(stated.bottom);
  });
});
