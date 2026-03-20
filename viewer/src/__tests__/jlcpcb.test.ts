import { describe, it, expect } from 'vitest';
import { parseSearchResult, extract3DModelUUID } from '../jlcpcb';
import { buildComponentSnippet } from '../jlcpcb-panel';

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
        description: '10kΩ ±1% 0805 Chip Resistor',
        images: [
          {
            '96x96': 'https://assets.lcsc.com/images/96x96/test_front.jpg',
            '224x224': 'https://assets.lcsc.com/images/224x224/test_front.jpg',
          },
        ],
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
    expect(result.imageUrl).toBe('https://assets.lcsc.com/images/224x224/test_front.jpg');
    expect(result.description).toBe('10kΩ ±1% 0805 Chip Resistor');
  });

  it('falls back to 96x96 when 224x224 is missing', () => {
    const raw = {
      lcsc: 100,
      mfr: 'TEST',
      package: '0402',
      stock: 5000,
      price: 0.01,
      extra: JSON.stringify({
        images: [{ '96x96': 'https://example.com/96.jpg' }],
      }),
    };

    const result = parseSearchResult(raw);
    expect(result.imageUrl).toBe('https://example.com/96.jpg');
  });

  it('returns empty imageUrl when no images array', () => {
    const raw = {
      lcsc: 200,
      mfr: 'PART',
      package: '0603',
      stock: 1000,
      price: 0.005,
      extra: JSON.stringify({ manufacturer: { name: 'Test' } }),
    };

    const result = parseSearchResult(raw);
    expect(result.imageUrl).toBe('');
    expect(result.description).toBe('');
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

  it('parses tiered price from JSON string', () => {
    const raw = {
      lcsc: 14663,
      mfr: 'CC0603KRX7R9BB104',
      package: '0603',
      stock: 81299425,
      price: JSON.stringify([
        { qFrom: 20, qTo: 19980, price: 0.002214 },
        { qFrom: 20000, qTo: 79980, price: 0.001643 },
      ]),
    };

    const result = parseSearchResult(raw);
    expect(result.price).toBeCloseTo(0.002214);
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

describe('buildComponentSnippet', () => {
  it('builds a resistor snippet with auto-numbered refdes', () => {
    const comp = parseSearchResult({
      lcsc: 17414,
      mfr: '0805W8F1002T5E',
      package: '0805',
      is_basic: true,
      stock: 15457503,
      price: 0.001642857,
      extra: JSON.stringify({
        manufacturer: { name: 'UNI-ROYAL' },
        attributes: { Resistance: '10kΩ', Tolerance: '±1%' },
        description: '10kΩ ±1% 0805 Chip Resistor',
      }),
    });

    // No existing components — should be R1
    const snippet = buildComponentSnippet(comp, []);
    expect(snippet).toContain('component R1');
    expect(snippet).toContain('resistor');
    expect(snippet).toContain('"0805"');
    expect(snippet).toContain('value "10kΩ"');
    expect(snippet).toContain('C17414');

    // With existing R1, R2 — should be R3
    const snippet2 = buildComponentSnippet(comp, ['R1', 'R2', 'C1']);
    expect(snippet2).toContain('component R3');
  });

  it('builds a capacitor snippet with correct numbering', () => {
    const comp = parseSearchResult({
      lcsc: 14663,
      mfr: 'CC0603KRX7R9BB104',
      package: '0603',
      is_basic: true,
      stock: 81299425,
      price: 0.002,
      extra: JSON.stringify({
        manufacturer: { name: 'YAGEO' },
        attributes: { Capacitance: '100nF', 'Voltage Rated': '50V' },
        description: '50V 100nF X7R Capacitor',
      }),
    });

    const snippet = buildComponentSnippet(comp, ['C1', 'C2', 'R1']);
    expect(snippet).toContain('component C3');
    expect(snippet).toContain('capacitor');
    expect(snippet).toContain('value "100nF"');
  });

  it('falls back to ic type for unknown components', () => {
    const comp = parseSearchResult({
      lcsc: 99999,
      mfr: 'MYSTERY-PART',
      package: 'QFN-48',
      stock: 100,
      price: 1.5,
      extra: JSON.stringify({
        manufacturer: { name: 'Unknown' },
        description: 'Some mystery component',
      }),
    });

    const snippet = buildComponentSnippet(comp, ['U1']);
    expect(snippet).toContain('component U2');
    expect(snippet).toContain('ic');
    expect(snippet).toContain('"QFN-48"');
  });

  it('starts at 1 when no existing components of same prefix', () => {
    const comp = parseSearchResult({
      lcsc: 100,
      mfr: 'LED-TEST',
      package: '0805',
      stock: 1000,
      price: 0.01,
      extra: JSON.stringify({ description: 'Green LED' }),
    });

    const snippet = buildComponentSnippet(comp, ['R1', 'C1', 'U1']);
    expect(snippet).toContain('component D1');
  });
});
