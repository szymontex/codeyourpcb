import { describe, it, expect, afterEach } from 'vitest';
import { encodeViewState, decodeViewState, type ViewState } from '../url-state';

/**
 * decodeViewState reads window.location.search, so we need to mock it.
 * We use Object.defineProperty to set a custom search string per test.
 */
function mockLocationSearch(search: string): void {
  Object.defineProperty(globalThis, 'window', {
    value: { location: { search } },
    writable: true,
    configurable: true,
  });
}

afterEach(() => {
  // Clean up window mock
  if ('window' in globalThis) {
    delete (globalThis as any).window;
  }
});

describe('encodeViewState', () => {
  it('encodes a typical view state to query string', () => {
    const state: ViewState = {
      layers: ['top', 'bottom', 'ratsnest'],
      zoom: 1.5,
      panX: 1000,
      panY: -2000,
    };
    const qs = encodeViewState(state);
    expect(qs).toContain('l=top%2Cbottom%2Cratsnest');
    expect(qs).toContain('z=1.50');
    expect(qs).toContain('x=1000');
    expect(qs).toContain('y=-2000');
    expect(qs.startsWith('?')).toBe(true);
  });

  it('handles single layer', () => {
    const state: ViewState = { layers: ['top'], zoom: 1, panX: 0, panY: 0 };
    const qs = encodeViewState(state);
    expect(qs).toContain('l=top');
  });

  it('rounds panX and panY to integers', () => {
    const state: ViewState = { layers: ['top'], zoom: 1, panX: 123.456, panY: -789.999 };
    const qs = encodeViewState(state);
    expect(qs).toContain('x=123');
    expect(qs).toContain('y=-790');
  });

  it('handles zero values', () => {
    const state: ViewState = { layers: ['top'], zoom: 0, panX: 0, panY: 0 };
    const qs = encodeViewState(state);
    expect(qs).toContain('z=0.00');
    expect(qs).toContain('x=0');
    expect(qs).toContain('y=0');
  });
});

describe('encodeViewState / decodeViewState roundtrip', () => {
  it('roundtrips a typical state', () => {
    const original: ViewState = {
      layers: ['top', 'bottom'],
      zoom: 2.5,
      panX: 5000,
      panY: -3000,
    };
    const qs = encodeViewState(original);
    mockLocationSearch(qs);
    const decoded = decodeViewState();

    expect(decoded).not.toBeNull();
    expect(decoded!.layers).toEqual(['top', 'bottom']);
    expect(decoded!.zoom).toBeCloseTo(2.5, 1);
    expect(decoded!.panX).toEqual(5000);
    expect(decoded!.panY).toEqual(-3000);
  });

  it('handles large coordinate values', () => {
    const original: ViewState = {
      layers: ['top'],
      zoom: 0.001,
      panX: 999_999_999,
      panY: -999_999_999,
    };
    const qs = encodeViewState(original);
    mockLocationSearch(qs);
    const decoded = decodeViewState();

    expect(decoded).not.toBeNull();
    expect(decoded!.panX).toEqual(999999999);
    expect(decoded!.panY).toEqual(-999999999);
  });
});

describe('decodeViewState', () => {
  it('returns null when no layer param present', () => {
    mockLocationSearch('?z=1&x=0&y=0');
    const result = decodeViewState();
    expect(result).toBeNull();
  });

  it('returns defaults for missing numeric params', () => {
    mockLocationSearch('?l=top');
    const result = decodeViewState();
    expect(result).not.toBeNull();
    expect(result!.layers).toEqual(['top']);
    expect(result!.zoom).toEqual(1);   // default
    expect(result!.panX).toEqual(0);   // default
    expect(result!.panY).toEqual(0);   // default
  });

  it('handles negative zoom gracefully', () => {
    mockLocationSearch('?l=top&z=-5&x=0&y=0');
    const result = decodeViewState();
    expect(result).not.toBeNull();
    expect(result!.zoom).toEqual(-5); // no clamping in decode — caller's responsibility
  });
});
