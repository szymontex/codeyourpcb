import { describe, it, expect } from 'vitest';
import { padDrills } from '../renderer3d';
import type { ComponentInfo } from '../types';

/**
 * The 3D view built drilled cylinders for vias and for nothing else.
 *
 * So a board full of through-hole parts came out solid: a connector's pins
 * showed their copper on the top face and on the bottom face with no hole
 * between them, and a mounting hole - which has no copper at all - showed
 * nothing whatsoever, on a view whose whole job is telling you whether the
 * thing fits in its enclosure.
 *
 * What is checked here is the decision, not the three.js: which pads are
 * drilled, where the holes land once the part is rotated, and how wide they
 * are. The mesh built from that is a template cylinder scaled per hole, the
 * same construction the vias already use.
 */
function component(
  refdes: string,
  x_nm: number,
  y_nm: number,
  rotation_mdeg: number,
  pads: ComponentInfo['pads'],
): ComponentInfo {
  return {
    refdes,
    value: '',
    footprint: 'test',
    x_nm,
    y_nm,
    rotation_mdeg,
    pads,
  } as ComponentInfo;
}

/** A 2.54mm-pitch pair of plated pins, both drilled 1mm. */
const HEADER = component('J1', 10_000_000, 10_000_000, 0, [
  {
    number: '1',
    x_nm: 0,
    y_nm: 0,
    width_nm: 1_700_000,
    height_nm: 1_700_000,
    shape: 'circle',
    layer_mask: 0x03,
    drill_nm: 1_000_000,
  },
  {
    number: '2',
    x_nm: 2_540_000,
    y_nm: 0,
    width_nm: 1_700_000,
    height_nm: 1_700_000,
    shape: 'circle',
    layer_mask: 0x03,
    drill_nm: 1_000_000,
  },
]);

/** A surface-mount part, which is drilled nowhere. */
const RESISTOR = component('R1', 20_000_000, 10_000_000, 0, [
  {
    number: '1',
    x_nm: -500_000,
    y_nm: 0,
    width_nm: 600_000,
    height_nm: 500_000,
    shape: 'rect',
    layer_mask: 0x01,
    drill_nm: null,
  },
  {
    number: '2',
    x_nm: 500_000,
    y_nm: 0,
    width_nm: 600_000,
    height_nm: 500_000,
    shape: 'rect',
    layer_mask: 0x01,
    drill_nm: null,
  },
]);

/** An M3 mounting hole: drilled 3.2mm, on no copper layer. */
const MOUNTING_HOLE = component('H1', 4_000_000, 4_000_000, 0, [
  {
    number: '',
    x_nm: 0,
    y_nm: 0,
    width_nm: 3_200_000,
    height_nm: 3_200_000,
    shape: 'circle',
    layer_mask: 0,
    drill_nm: 3_200_000,
  },
]);

describe('holes in the 3D view', () => {
  it('drills every pad that has a drill and none that has not', () => {
    const drills = padDrills([HEADER, RESISTOR, MOUNTING_HOLE]);

    expect(
      drills.length,
      'two header pins and one mounting hole are drilled; the resistor is not',
    ).toBe(3);
  });

  it('drills a mounting hole, which no copper-driven pass would reach', () => {
    const drills = padDrills([MOUNTING_HOLE]);

    expect(drills.length).toBe(1);
    expect(drills[0].diameter).toBeCloseTo(3.2, 6);
    expect(drills[0].x).toBeCloseTo(4.0, 6);
    expect(drills[0].y).toBeCloseTo(4.0, 6);
  });

  it('puts a hole where the part was turned, not where it was drawn', () => {
    // The second pin sits 2.54mm along the part's own x. Turn the part a
    // quarter, and that pin is 2.54mm along the board's y instead. A hole
    // drilled at the untouched offset lands under the neighbouring pad.
    const turned = component('J1', 10_000_000, 10_000_000, 90_000, HEADER.pads);
    const drills = padDrills([turned]);

    expect(drills.length).toBe(2);
    expect(drills[0].x).toBeCloseTo(10.0, 6);
    expect(drills[0].y).toBeCloseTo(10.0, 6);
    expect(drills[1].x, 'the turned pin keeps the board x of its origin').toBeCloseTo(10.0, 6);
    expect(drills[1].y, 'and moves along the board y instead').toBeCloseTo(12.54, 6);
  });

  it('drills each hole at the size the pad asks for', () => {
    const drills = padDrills([HEADER, MOUNTING_HOLE]);
    const diameters = drills.map((d) => d.diameter).sort((a, b) => a - b);

    // Compared with a tolerance, because nanometres reach millimetres through
    // a float divide: 3.2mm comes back as 3.1999999999999997.
    expect(diameters.length).toBe(3);
    expect(diameters[0]).toBeCloseTo(1, 6);
    expect(diameters[1]).toBeCloseTo(1, 6);
    expect(diameters[2]).toBeCloseTo(3.2, 6);
  });

  it('has nothing to drill on a board of surface-mount parts', () => {
    expect(padDrills([RESISTOR])).toEqual([]);
    expect(padDrills([])).toEqual([]);
  });
});
