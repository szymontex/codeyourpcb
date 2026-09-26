import { describe, it, expect } from 'vitest';
import { parseEasyEDAFootprint } from '../easyeda-footprint-parser';
import ap2112k from './fixtures/easyeda-ap2112k-sot25.json';

// EasyEDA's Y grows down the screen and a footprint's grows up. A part read
// without turning Y round is its own mirror image: the pins count the wrong
// way round, and the pin-1 dot sits on the far side of the body from pin 1
// once the part is soldered. The fixture is written by hand in EasyEDA's
// response format: pads from the SOT25 Suggested Pad Layout in DS39724
// Rev. 2-2, page 14 (C1 2.40, C2 0.95, X 0.55, Y 0.80 mm), body outline from
// its Package Outline (H 3.00, B 1.60 mm typ), read 2026-09-26. The pin-1
// dot's place is the test's own choice.

/** Twice the signed area the pins enclose, joined in order: positive when
 *  they run counter-clockwise in a Y-up frame. The same measure the built-in
 *  footprints are held to in `every_builtin_footprint_counts_its_pins_counter_clockwise`. */
function winding(pins: [number, number][]): number {
  return pins.reduce((sum, [x0, y0], i) => {
    const [x1, y1] = pins[(i + 1) % pins.length];
    return sum + x0 * y1 - x1 * y0;
  }, 0);
}

function imported() {
  const fp = parseEasyEDAFootprint(ap2112k);
  expect(fp).not.toBeNull();
  return fp!;
}

function pinsInOrder(): [number, number][] {
  return [...imported().pads]
    .sort((a, b) => Number(a.number) - Number(b.number))
    .map((pad) => [pad.x_nm, pad.y_nm]);
}

describe('a part fetched from EasyEDA', () => {
  it('counts its pins counter-clockwise seen from the top, as the datasheet does', () => {
    // DS39724 Rev. 2-2, SOT25 top view: 1 VIN, 2 GND, 3 EN down the left
    // side, 4 NC and 5 VOUT up the right - counter-clockwise.
    const pins = pinsInOrder();
    expect(pins).toHaveLength(5);
    expect(winding(pins)).toBeGreaterThan(0);
  });

  it('puts pin 1 where EasyEDA draws it, below and left of the body centre', () => {
    // The fixture lays the part out as EasyEDA does: the datasheet's top view
    // turned a quarter turn counter-clockwise, pins 1 to 3 along the bottom
    // edge, pin 1 on the left.
    const pins = pinsInOrder();
    const cx = pins.reduce((s, [x]) => s + x, 0) / pins.length;
    const cy = pins.reduce((s, [, y]) => s + y, 0) / pins.length;
    const [x1, y1] = pins[0];
    expect(x1).toBeLessThan(cx);
    expect(y1).toBeLessThan(cy);
  });

  it('keeps its pin-1 dot next to pin 1', () => {
    const fp = imported();
    const dot = fp.silk.find((s) => s.type === 'arc');
    expect(dot).toBeDefined();
    const { cx, cy } = dot as { cx: number; cy: number };
    const nearest = [...fp.pads].sort(
      (a, b) => Math.hypot(a.x_nm - cx, a.y_nm - cy) - Math.hypot(b.x_nm - cx, b.y_nm - cy),
    )[0];
    expect(nearest.number).toBe('1');
  });

  it('turns its silkscreen lines and circles the same way as its pads', () => {
    // A line and a circle drawn 10 units below the origin on EasyEDA's
    // screen are 2.54 mm below it on the board.
    const fp = parseEasyEDAFootprint({
      result: {
        packageDetail: {
          dataStr: {
            head: { x: '400', y: '300' },
            shape: [
              'PAD~RECT~400~310~4~4~1~~1~0~~0~gge1',
              'TRACK~1~3~~395 310 405 310~gge2~0',
              'CIRCLE~400~310~2~1~3~gge3~0~~',
            ],
          },
        },
      },
    })!;
    expect(fp.pads[0].y_nm).toBe(-2_540_000);
    const line = fp.silk.find((s) => s.type === 'segment') as { y1: number; y2: number };
    expect([line.y1, line.y2]).toEqual([-2_540_000, -2_540_000]);
    const circle = fp.silk.find((s) => s.type === 'circle') as { cy: number };
    expect(circle.cy).toBe(-2_540_000);
  });

  it('draws a silkscreen arc through the same points, counter-clockwise from start to end', () => {
    // On EasyEDA's screen this quarter runs clockwise from the right of the
    // origin to below it. On the board it is the quarter from straight down
    // (-90 degrees) round to the right (0 degrees).
    const fp = parseEasyEDAFootprint({
      result: {
        packageDetail: {
          dataStr: {
            head: { x: '400', y: '300' },
            shape: [
              'PAD~RECT~400~300~4~4~1~~1~0~~0~gge1',
              'ARC~1~3~~M 410 300 A 10 10 0 0 1 400 310~~gge2~0',
            ],
          },
        },
      },
    })!;
    const arc = fp.silk.find((s) => s.type === 'arc') as {
      cx: number; cy: number; startAngle: number; endAngle: number;
    };
    expect(Math.abs(arc.cx)).toBeLessThan(1);
    expect(Math.abs(arc.cy)).toBeLessThan(1);
    expect(arc.startAngle).toBeCloseTo(-Math.PI / 2, 6);
    expect(arc.endAngle).toBeCloseTo(0, 6);
  });
});
