import { describe, it, expect } from 'vitest';
import { parseEasyEDAFootprint } from '../easyeda-footprint-parser';

describe('parseEasyEDAFootprint', () => {
  it('parses a simple 2-pad footprint from LIB block', () => {
    // Simulates EasyEDA response for a 0805 resistor
    // PAD format: PAD~SHAPE~X~Y~WIDTH~HEIGHT~LAYERID~NET~NUMBER~HOLER~...~GID
    const compData = {
      result: {
        packageDetail: {
          dataStr: {
            shape: [
              'LIB~400~300~package`0805`~~~gge1~1' +
              '#@$PAD~RECT~396.26~300~3.94~5.71~1~~1~0~~0~gge2' +
              '#@$PAD~RECT~403.74~300~3.94~5.71~1~~2~0~~0~gge3',
            ],
          },
        },
      },
    };

    const fp = parseEasyEDAFootprint(compData);
    expect(fp).not.toBeNull();
    expect(fp!.pads).toHaveLength(2);

    // Pad 1: relative X = (396.26 - 400) * 254000 = -950_440 nm
    expect(fp!.pads[0].number).toBe('1');
    expect(fp!.pads[0].x_nm).toBeCloseTo(-950_440, -3); // ~950,000 nm
    expect(fp!.pads[0].y_nm).toBe(0);
    expect(fp!.pads[0].shape).toBe('rect');
    expect(fp!.pads[0].layer_mask).toBe(1); // Top only (SMD)
    expect(fp!.pads[0].drill_nm).toBeNull();

    // Pad 2: relative X = (403.74 - 400) * 254000 = +949,960 nm (after rounding)
    expect(fp!.pads[1].number).toBe('2');
    expect(fp!.pads[1].x_nm).toBeCloseTo(949_960, -3);
  });

  it('parses through-hole pads with drill', () => {
    const compData = {
      result: {
        packageDetail: {
          dataStr: {
            shape: [
              'LIB~400~300~package`DIP-8`~~~gge1~1' +
              '#@$PAD~ELLIPSE~385~315~6.7~6.7~11~~1~1.6~~0~gge2' +
              '#@$PAD~ELLIPSE~385~305~6.7~6.7~11~~2~1.6~~0~gge3',
            ],
          },
        },
      },
    };

    const fp = parseEasyEDAFootprint(compData);
    expect(fp).not.toBeNull();
    expect(fp!.pads).toHaveLength(2);

    // Through-hole: layer 11 → layer_mask 3 (both sides)
    expect(fp!.pads[0].layer_mask).toBe(3);
    expect(fp!.pads[0].shape).toBe('circle');

    // Drill: holeR=1.6 → diameter = 1.6 * 2 * 254000 = 812,800 nm
    expect(fp!.pads[0].drill_nm).toBeCloseTo(812_800, -3);
  });

  it('extracts 3D model UUID from shape array', () => {
    const compData = {
      result: {
        packageDetail: {
          dataStr: {
            shape: [
              'LIB~400~300~package`0805`~~~gge1~1' +
              '#@$PAD~RECT~396~300~4~6~1~~1~0~~0~gge2' +
              '#@$PAD~RECT~404~300~4~6~1~~2~0~~0~gge3',
              'SVGNODE~{"gId":"g1","attrs":{"c_etype":"outline3D","uuid":"c7acac53bcbc44d68fbab8f60a747688","title":"0805"}}',
            ],
          },
        },
      },
    };

    const fp = parseEasyEDAFootprint(compData);
    expect(fp).not.toBeNull();
    expect(fp!.modelUuid).toBe('c7acac53bcbc44d68fbab8f60a747688');
    expect(fp!.pads).toHaveLength(2);
  });

  it('returns null for empty/missing response', () => {
    expect(parseEasyEDAFootprint(null)).toBeNull();
    expect(parseEasyEDAFootprint({})).toBeNull();
    expect(parseEasyEDAFootprint({ result: null })).toBeNull();
    expect(parseEasyEDAFootprint({ result: { packageDetail: null } })).toBeNull();
  });

  it('returns null when no PAD shapes found', () => {
    const compData = {
      result: {
        packageDetail: {
          dataStr: {
            shape: [
              'TRACK~1~1~S$7~311 175 351 175~gge6',
            ],
          },
        },
      },
    };

    expect(parseEasyEDAFootprint(compData)).toBeNull();
  });

  it('handles result as array', () => {
    const compData = {
      result: [
        {
          packageDetail: {
            dataStr: {
              shape: [
                'LIB~400~300~package`SOT-23`~~~gge1~1' +
                '#@$PAD~RECT~396~296~2.36~3.94~1~~1~0~~0~gge2' +
                '#@$PAD~RECT~404~296~2.36~3.94~1~~2~0~~0~gge3' +
                '#@$PAD~RECT~400~304~2.36~3.94~1~~3~0~~0~gge4',
              ],
            },
          },
        },
      ],
    };

    const fp = parseEasyEDAFootprint(compData);
    expect(fp).not.toBeNull();
    expect(fp!.pads).toHaveLength(3);
  });

  it('computes correct origin-relative coordinates', () => {
    // LIB origin at 500, 400. Pad at 510, 400.
    // Relative: (510-500)*254000 = 2,540,000 nm = 2.54mm
    const compData = {
      result: {
        packageDetail: {
          dataStr: {
            shape: [
              'LIB~500~400~package`TEST`~~~gge1~1' +
              '#@$PAD~RECT~510~400~4~4~1~~1~0~~0~gge2',
            ],
          },
        },
      },
    };

    const fp = parseEasyEDAFootprint(compData);
    expect(fp).not.toBeNull();
    expect(fp!.pads[0].x_nm).toBe(2_540_000);
    expect(fp!.pads[0].y_nm).toBe(0);
  });

  it('parses v6 standalone PADs using head origin', () => {
    // EasyEDA v6 format: PADs are top-level shapes, origin in head.x/head.y
    // This matches real API response for C17414 (0805 resistor)
    const compData = {
      result: {
        packageDetail: {
          dataStr: {
            head: {
              docType: '4',
              x: 4000,
              y: 3000,
            },
            shape: [
              'SOLIDREGION~100~~M 3996 3002 L 3996 2997~solid~gge100',
              'PAD~RECT~4003.937~3000~4.4588~5.4213~1~~2~0~4001 3002 4001 2997 4006 2997 4006 3002~0~gge1002~0~~Y~0',
              'PAD~RECT~3996.063~3000~4.4588~5.4213~1~~1~0~3998 3002 3998 2997 3993 2997 3993 3002~0~gge1004~0~~Y~0',
              'SVGNODE~{"gId":"g1","attrs":{"c_etype":"outline3D","uuid":"c7acac53bcbc44d68fbab8f60a747688"}}',
            ],
          },
        },
      },
    };

    const fp = parseEasyEDAFootprint(compData);
    expect(fp).not.toBeNull();
    expect(fp!.pads).toHaveLength(2);

    // Pad 1 (number='1'): X = (3996.063 - 4000) * 254000 = -1,000,002 nm ≈ -1mm
    const pad1 = fp!.pads.find(p => p.number === '1')!;
    expect(pad1).toBeDefined();
    expect(pad1.x_nm).toBeCloseTo(-1_000_000, -4);
    expect(pad1.y_nm).toBe(0);
    expect(pad1.shape).toBe('rect');
    expect(pad1.layer_mask).toBe(1); // SMD top

    // Pad 2 (number='2'): X = (4003.937 - 4000) * 254000 = +999,998 nm ≈ +1mm
    const pad2 = fp!.pads.find(p => p.number === '2')!;
    expect(pad2).toBeDefined();
    expect(pad2.x_nm).toBeCloseTo(1_000_000, -4);

    // 3D model UUID
    expect(fp!.modelUuid).toBe('c7acac53bcbc44d68fbab8f60a747688');
  });
});
