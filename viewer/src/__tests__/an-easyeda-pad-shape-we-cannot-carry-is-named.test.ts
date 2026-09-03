import { describe, it, expect } from 'vitest';
import { parseEasyEDAFootprint } from '../easyeda-footprint-parser';

/**
 * A pad shape this parser cannot carry is named, not swallowed.
 *
 * `npx vitest run src/__tests__/an-easyeda-pad-shape-we-cannot-carry-is-named.test.ts`
 *
 * EasyEDA writes POLYGON for a pad somebody drew. This parser answered it with
 * `shape = 'rect'; // approximate` and said nothing, and a part placed from the
 * JLCPCB panel brings its pads onto the board - so the substitution is copper:
 * the checker measures the rectangle, the router blocks it, the Gerber flashes
 * it.
 *
 * Approximating is still the right answer; a panel that refuses a part because
 * one pad is a polygon is a panel nobody uses. Saying nothing was the fault.
 */

/** `PAD~SHAPE~X~Y~W~H~LAYER~NET~NUM~HOLER~...` */
function pad(shape: string, number: string): string {
  return `PAD~${shape}~100~100~20~20~1~~${number}~0~~gid`;
}

function component(...shapes: string[]) {
  return { result: { packageDetail: { dataStr: { head: { x: '0', y: '0' }, shape: shapes } } } };
}

describe('parseEasyEDAFootprint', () => {
  it('names the pad whose shape it had to approximate', () => {
    const fp = parseEasyEDAFootprint(component(pad('RECT', '1'), pad('POLYGON', '2')));

    expect(fp).not.toBeNull();
    expect(fp!.pads).toHaveLength(2);
    expect(fp!.approximations).toHaveLength(1);
    expect(fp!.approximations[0]).toContain('pad 2');
    expect(fp!.approximations[0]).toContain('POLYGON');
  });

  it('says nothing about the shapes it does carry', () => {
    // The control. A warning on every part is a warning nobody reads, and an
    // absence proves nothing unless the same parser can produce one.
    const fp = parseEasyEDAFootprint(
      component(pad('RECT', '1'), pad('ELLIPSE', '2'), pad('OVAL', '3')),
    );

    expect(fp).not.toBeNull();
    expect(fp!.pads).toHaveLength(3);
    expect(fp!.approximations).toEqual([]);
  });

  it('reads an ELLIPSE as a circle whether or not it is drilled', () => {
    // This arm used to read `drillNm ? 'circle' : 'circle'`: both branches the
    // same, which is a question somebody meant to ask and never did. Both
    // answers are asserted so removing the ternary cannot have changed one.
    const smd = parseEasyEDAFootprint(component(pad('ELLIPSE', '1')));
    const drilled = parseEasyEDAFootprint(
      component(`PAD~ELLIPSE~100~100~20~20~11~~1~5~~gid`),
    );

    expect(smd!.pads[0].shape).toBe('circle');
    expect(drilled!.pads[0].shape).toBe('circle');
    expect(drilled!.pads[0].drill_nm).toBeGreaterThan(0);
  });
});
