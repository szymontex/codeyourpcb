import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { parseEasyEDAFootprint } from '../easyeda-footprint-parser';
import offsetHole from './fixtures/easyeda-offset-hole.json';
import usb4105 from './fixtures/easyeda-usb4105-holes.json';

// A pad's hole is not always in the middle of its copper. EasyEDA's PAD states
// it as the twentieth field, `holeCenter`, the hole's own `x,y` in the same
// absolute coordinates as the pad's (the format document's example,
// `PAD~ELLIPSE~4020~3308.5~6~6~11~~1~1.8~~0~gge5~0~~Y~0~~~4020,3308.5`, states
// the pad's centre again; read 2026-10-11). The parser read the fields up to
// the slot and stopped: a pad with its hole moved arrived with the hole in the
// middle, and the drill file put it there.
//
// The fixture is synthetic, written from that example: three 2.00 x 3.00mm pads
// with a 0.80mm hole, the first with its hole 0.5mm right and 0.5mm down the
// sheet from its middle, the second the same turned 90 degrees, the third with
// its hole in the middle. `crates/cypcb-render/tests/fixtures/
// easyeda-offset-hole-pads.json` is what this parser writes for it, and the
// Rust test `a_hole_off_the_middle_of_its_pad_is_drilled_where_it_is` takes the
// same file to the drill file.

const NM = 100; // EasyEDA writes four decimals of 0.254mm: within 100nm.

const contract = (name: string) =>
  JSON.parse(
    readFileSync(resolve(__dirname, '../../../crates/cypcb-render/tests/fixtures', name), 'utf8'),
  );

const pads = () => parseEasyEDAFootprint(offsetHole)!.pads;

describe('an EasyEDA pad with its hole off the middle of its copper', () => {
  it('states the hole as the distance from the pad, in this project\'s frame', () => {
    const [first] = pads();
    // Right 0.5mm, and down the sheet 0.5mm: EasyEDA's y grows downwards and
    // this project's grows upwards, so the offset is -0.5mm in y.
    expect(Math.abs(first.hole_offset_nm![0] - 500_000)).toBeLessThan(NM);
    expect(Math.abs(first.hole_offset_nm![1] + 500_000)).toBeLessThan(NM);
    // The copper stays where it was drawn.
    expect([first.x_nm, first.y_nm]).toEqual([0, 0]);
  });

  it('does not turn the offset with the pad: holeCenter is absolute', () => {
    const [first, turned] = pads();
    // Pad 2 is pad 1 turned 90 degrees: its sides swap, and its hole is the
    // same 0.5mm right and 0.5mm down the sheet, because the file states the
    // hole's own point and not a vector in the pad's frame.
    expect([turned.width_nm, turned.height_nm]).toEqual([first.height_nm, first.width_nm]);
    expect(turned.hole_offset_nm).toEqual(first.hole_offset_nm);
  });

  it('states nothing for a hole in the middle: the control', () => {
    const [, , middle] = pads();
    expect(middle.drill_nm).not.toBeNull();
    expect(middle.hole_offset_nm).toBeUndefined();
  });

  it('states nothing for the pads of a part whose holes are all in the middle', () => {
    // The USB4105 fixture is written with holeCenter equal to each pad's
    // centre, four decimals and all.
    for (const pad of parseEasyEDAFootprint(usb4105)!.pads) {
      expect(pad.hole_offset_nm).toBeUndefined();
    }
  });

  it('is the file the engine is held to', () => {
    expect(pads()).toEqual(contract('easyeda-offset-hole-pads.json'));
  });
});
