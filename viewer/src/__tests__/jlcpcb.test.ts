import { describe, it, expect } from 'vitest';
import { parseSearchResult, extract3DModelUUID } from '../jlcpcb';

describe('parseSearchResult', () => {
  it('parses a valid search result with extra JSON', () => {
    const raw = {
      lcsc: 17414,
      mfr: '0805W8F1002T5E',
      package: '0805',
      is_basic: true,
      stock: 15457503,
      price: 0.001642857,
      extra: JSON.stringify({
        manufacturer: { name: 'UNI-ROYAL' },
        attributes: { Resistance: '10kΩ', Tolerance: '±1%' },
        datasheet: { pdf: 'https://example.com/datasheet.pdf' },
      }),
    };

    const result = parseSearchResult(raw);

    expect(result.lcsc).toBe(17414);
    expect(result.mfr).toBe('0805W8F1002T5E');
    expect(result.package).toBe('0805');
    expect(result.isBasic).toBe(true);
    expect(result.stock).toBe(15457503);
    expect(result.price).toBeCloseTo(0.001642857);
    expect(result.manufacturer).toBe('UNI-ROYAL');
    expect(result.attributes.Resistance).toBe('10kΩ');
    expect(result.attributes.Tolerance).toBe('±1%');
    expect(result.datasheetUrl).toBe('https://example.com/datasheet.pdf');
  });

  it('handles empty components array gracefully', () => {
    const raw = {
      lcsc: 0,
      mfr: '',
      package: '',
      stock: 0,
      price: 0,
    };

    const result = parseSearchResult(raw);
    expect(result.lcsc).toBe(0);
    expect(result.mfr).toBe('');
    expect(result.manufacturer).toBe('');
    expect(result.attributes).toEqual({});
    expect(result.datasheetUrl).toBe('');
  });

  it('handles malformed extra JSON string', () => {
    const raw = {
      lcsc: 100,
      mfr: 'TEST',
      package: '0402',
      stock: 5000,
      price: 0.01,
      extra: 'not valid json {{{',
    };

    const result = parseSearchResult(raw);
    expect(result.lcsc).toBe(100);
    expect(result.manufacturer).toBe('');
    expect(result.attributes).toEqual({});
  });

  it('handles missing extra field', () => {
    const raw = {
      lcsc: 200,
      mfr: 'PART',
      package: '0603',
      stock: 1000,
      price: 0.005,
    };

    const result = parseSearchResult(raw);
    expect(result.manufacturer).toBe('');
    expect(result.datasheetUrl).toBe('');
  });
});

describe('extract3DModelUUID', () => {
  it('extracts UUID from EasyEDA shape array with outline3D', () => {
    const compData = {
      result: {
        packageDetail: {
          dataStr: {
            shape: [
              'SVGNODE~{"gId":"g1","nodeName":"g"}',
              'SVGNODE~{"gId":"g2","attrs":{"c_etype":"outline3D","uuid":"c7acac53bcbc44d68fbab8f60a747688","title":"0805"}}',
            ],
          },
        },
      },
    };

    const uuid = extract3DModelUUID(compData);
    expect(uuid).toBe('c7acac53bcbc44d68fbab8f60a747688');
  });

  it('returns null when no outline3D in shape array', () => {
    const compData = {
      result: {
        packageDetail: {
          dataStr: {
            shape: [
              'SVGNODE~{"gId":"g1","nodeName":"g"}',
              'SVGNODE~{"gId":"g2","attrs":{"c_etype":"outline","title":"0805"}}',
            ],
          },
        },
      },
    };

    const uuid = extract3DModelUUID(compData);
    expect(uuid).toBeNull();
  });

  it('returns null for missing result', () => {
    expect(extract3DModelUUID({})).toBeNull();
    expect(extract3DModelUUID(null)).toBeNull();
    expect(extract3DModelUUID({ result: null })).toBeNull();
  });

  it('returns null when packageDetail is missing', () => {
    const compData = {
      result: {
        packageDetail: null,
      },
    };

    expect(extract3DModelUUID(compData)).toBeNull();
  });

  it('handles result as array', () => {
    const compData = {
      result: [
        {
          packageDetail: {
            dataStr: {
              shape: [
                'SVGNODE~{"attrs":{"c_etype":"outline3D","uuid":"abcdef12345678901234567890abcdef"}}',
              ],
            },
          },
        },
      ],
    };

    const uuid = extract3DModelUUID(compData);
    expect(uuid).toBe('abcdef12345678901234567890abcdef');
  });
});
