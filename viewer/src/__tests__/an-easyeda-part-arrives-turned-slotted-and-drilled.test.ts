import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { parseEasyEDAFootprint } from '../easyeda-footprint-parser';
import usb4105 from './fixtures/easyeda-usb4105-holes.json';
import ap2112k from './fixtures/easyeda-ap2112k-sot25.json';

// Three things a part fetched from EasyEDA states and the parser dropped until
// 2026-09-26: a pad's rotation, a pad's slot, and a hole with no copper. The
// USB4105 fixture is written by hand from GCT's drawing (revision B4,
// 18/12/23): two Ø0.65 non-plated pegs 5.78 apart, and shell slots 0.60 x 1.70
// in 1.00 x 2.10 pads and 0.60 x 1.40 in 1.00 x 1.80 pads.
//
// The engine reads what this parser writes. `crates/cypcb-render/tests/
// fixtures/` holds that output for the USB4105 pads and for a quarter arc, and
// the Rust tests in `an_easyeda_part_reaches_the_fab_as_drawn.rs` take the same
// files to the drill file and the legend Gerber. Each side is checked against
// one file, so a field renamed on either side fails a test instead of being
// skipped.

const contract = (name: string) =>
  JSON.parse(
    readFileSync(resolve(__dirname, '../../../crates/cypcb-render/tests/fixtures', name), 'utf8'),
  );

/** The fixture with every shape passed through `edit`. */
function edited(fixture: any, edit: (shape: string) => string) {
  const copy = JSON.parse(JSON.stringify(fixture));
  copy.result.packageDetail.dataStr.shape = copy.result.packageDetail.dataStr.shape.map(edit);
  return copy;
}

/** A PAD with its rotation (field 11) replaced. */
const turned = (degrees: number) => (shape: string) => {
  if (!shape.startsWith('PAD~')) return shape;
  const fields = shape.split('~');
  fields[11] = String(degrees);
  return fields.join('~');
};

/** Twice the signed area the pins enclose: positive counter-clockwise. */
function winding(pins: [number, number][]): number {
  return pins.reduce((sum, [x0, y0], i) => {
    const [x1, y1] = pins[(i + 1) % pins.length];
    return sum + x0 * y1 - x1 * y0;
  }, 0);
}

const NM = 100; // EasyEDA writes four decimals of 0.254mm: within 100nm.

describe('a part fetched from EasyEDA', () => {
  it('brings its non-plated holes, at the size the drawing gives', () => {
    // A HOLE's size field is a radius: USB4105's pegs are written 1.2795,
    // a 0.325mm radius, for the drawing's Ø0.65.
    const holes = parseEasyEDAFootprint(usb4105)!.pads.filter((p) => p.layer_mask === 0);
    expect(holes).toHaveLength(2);
    for (const hole of holes) {
      expect(hole.drill_nm).not.toBeNull();
      expect(Math.abs(hole.drill_nm! - 650_000)).toBeLessThan(NM);
      expect(hole.shape).toBe('circle');
    }
    const [left, right] = [...holes].sort((a, b) => a.x_nm - b.x_nm);
    expect(Math.abs(right.x_nm - left.x_nm - 5_780_000)).toBeLessThan(NM);
  });

  it('brings its shell slots as slots, along the long side of their pads', () => {
    const shells = parseEasyEDAFootprint(usb4105)!.pads.filter((p) => p.number === 'SH');
    expect(shells).toHaveLength(4);
    const slots = shells.map((p) => [p.drill_nm, ...(p.slot_nm ?? [])].map((n) => Math.round(n! / 10_000) / 100));
    // 0.60 x 1.40mm and 0.60 x 1.70mm slots, running along 1.00 x 1.80mm and
    // 1.00 x 2.10mm pads.
    expect(slots.sort()).toEqual([
      [0.6, 0.6, 1.4], [0.6, 0.6, 1.4], [0.6, 0.6, 1.7], [0.6, 0.6, 1.7],
    ]);
    const mm = (nm: number) => Math.round(nm / 10_000) / 100;
    expect(shells.map((p) => [mm(p.width_nm), mm(p.height_nm)]).sort()).toEqual([
      [1, 1.8], [1, 1.8], [1, 2.1], [1, 2.1],
    ]);
  });

  it('turns a pad and its slot a quarter turn by swapping their sides, in place', () => {
    const straight = parseEasyEDAFootprint(usb4105)!.pads.filter((p) => p.number === 'SH');
    for (const degrees of [90, 270, -90]) {
      const quarter = parseEasyEDAFootprint(edited(usb4105, turned(degrees)))!;
      const shells = quarter.pads.filter((p) => p.number === 'SH');
      shells.forEach((pad, i) => {
        const was = straight[i];
        expect([pad.x_nm, pad.y_nm]).toEqual([was.x_nm, was.y_nm]);
        expect([pad.width_nm, pad.height_nm]).toEqual([was.height_nm, was.width_nm]);
        expect(pad.slot_nm).toEqual([was.slot_nm![1], was.slot_nm![0]]);
        expect(pad.drill_nm).toBe(was.drill_nm);
      });
      expect(quarter.approximations).toEqual([]);
    }
    const half = parseEasyEDAFootprint(edited(usb4105, turned(180)))!.pads.filter((p) => p.number === 'SH');
    expect(half).toEqual(straight);
  });

  it('keeps counting its pins counter-clockwise when every pad is turned', () => {
    for (const degrees of [90, 45]) {
      const fp = parseEasyEDAFootprint(edited(ap2112k, turned(degrees)))!;
      const pins = [...fp.pads]
        .sort((a, b) => Number(a.number) - Number(b.number))
        .map((pad) => [pad.x_nm, pad.y_nm] as [number, number]);
      expect(winding(pins)).toBeGreaterThan(0);
    }
  });

  it('swaps a SOT25 pad to 0.80 x 0.55 at 90 degrees', () => {
    const fp = parseEasyEDAFootprint(edited(ap2112k, turned(90)))!;
    for (const pad of fp.pads) {
      expect(Math.abs(pad.width_nm - 800_000)).toBeLessThan(NM);
      expect(Math.abs(pad.height_nm - 550_000)).toBeLessThan(NM);
    }
    expect(fp.approximations).toEqual([]);
  });

  it('names every pad turned by an angle it cannot draw, and leaves it unturned', () => {
    const fp = parseEasyEDAFootprint(edited(ap2112k, turned(45)))!;
    const straight = parseEasyEDAFootprint(ap2112k)!;
    expect(fp.pads).toEqual(straight.pads);
    expect([...fp.approximations].sort()).toEqual(
      ['1', '2', '3', '4', '5'].map((n) => `pad ${n} states rotation 45`),
    );
  });

  it('writes its pads exactly as the engine reads them', () => {
    expect(parseEasyEDAFootprint(usb4105)!.pads).toEqual(contract('easyeda-usb4105-pads.json'));
  });

  it('writes a quarter arc exactly as the engine reads it', () => {
    // A quarter from the right of the origin to above it, on the board.
    const fp = parseEasyEDAFootprint({
      result: {
        packageDetail: {
          dataStr: {
            head: { x: '4000', y: '3000' },
            shape: [
              'PAD~RECT~4000~3000~4~4~1~~1~0~~0~gge1',
              'ARC~1~3~~M 4010 3000 A 10 10 0 0 0 4000 2990~~gge2~0',
            ],
          },
        },
      },
    })!;
    expect(fp.silk).toEqual(contract('easyeda-quarter-arc-silk.json'));
  });
});
