import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

/**
 * Settings module tests.
 *
 * The settings module runs side-effects at import time (reads localStorage,
 * sets window.__settings). We need to set up the mock environment BEFORE
 * importing, then re-import for each test to get clean state.
 */

// ---------------------------------------------------------------------------
// Helpers: mock localStorage + window for node environment
// ---------------------------------------------------------------------------

let storage: Record<string, string> = {};

const mockLocalStorage = {
  getItem: vi.fn((key: string) => storage[key] ?? null),
  setItem: vi.fn((key: string, value: string) => { storage[key] = value; }),
  removeItem: vi.fn((key: string) => { delete storage[key]; }),
  clear: vi.fn(() => { storage = {}; }),
};

function setupGlobals() {
  (globalThis as any).localStorage = mockLocalStorage;
  if (typeof globalThis.window === 'undefined') {
    (globalThis as any).window = {};
  }
}

function teardownGlobals() {
  delete (globalThis as any).localStorage;
  delete (globalThis as any).window;
}

// We need a fresh module per test since the module has top-level state.
async function importSettings() {
  // Bust the module cache so we get fresh state on each import
  const mod = await import('../settings');
  return mod;
}

beforeEach(() => {
  storage = {};
  vi.clearAllMocks();
  setupGlobals();
  vi.resetModules();
});

afterEach(() => {
  teardownGlobals();
});

describe('settings', () => {
  it('returns DEFAULT_SETTINGS when localStorage is empty', async () => {
    const { getSettings, DEFAULT_SETTINGS } = await importSettings();
    expect(getSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it('get/set round-trip for a simple preference', async () => {
    const { getPreference, setPreference } = await importSettings();
    expect(getPreference('units')).toBe('mm');

    setPreference('units', 'mil');
    expect(getPreference('units')).toBe('mil');
  });

  it('persists changes to localStorage', async () => {
    const { setPreference } = await importSettings();

    setPreference('gridVisible', false);

    const stored = JSON.parse(storage['cypcb-settings']);
    expect(stored.gridVisible).toBe(false);
  });

  it('subscribe notifies on change', async () => {
    const { subscribe, setPreference } = await importSettings();

    const listener = vi.fn();
    subscribe(listener);

    setPreference('units', 'mil');

    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener.mock.calls[0][0].units).toBe('mil');
  });

  it('supports multiple subscribers', async () => {
    const { subscribe, setPreference } = await importSettings();

    const listener1 = vi.fn();
    const listener2 = vi.fn();
    subscribe(listener1);
    subscribe(listener2);

    setPreference('gridVisible', false);

    expect(listener1).toHaveBeenCalledTimes(1);
    expect(listener2).toHaveBeenCalledTimes(1);
  });

  it('unsubscribe stops notifications', async () => {
    const { subscribe, setPreference } = await importSettings();

    const listener = vi.fn();
    const unsub = subscribe(listener);

    setPreference('units', 'mil');
    expect(listener).toHaveBeenCalledTimes(1);

    unsub();
    setPreference('units', 'µm');
    // Should NOT have been called again
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('falls back to defaults when localStorage contains invalid JSON', async () => {
    storage['cypcb-settings'] = '{not valid json!!!';

    const { getSettings, DEFAULT_SETTINGS } = await importSettings();
    expect(getSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it('merges partial settings with defaults', async () => {
    storage['cypcb-settings'] = JSON.stringify({ units: 'mil' });

    const { getSettings, DEFAULT_SETTINGS } = await importSettings();
    const settings = getSettings();

    // Overridden value
    expect(settings.units).toBe('mil');
    // Default values still present
    expect(settings.gridVisualSpacing).toBe(DEFAULT_SETTINGS.gridVisualSpacing);
    expect(settings.layerColors).toEqual(DEFAULT_SETTINGS.layerColors);
  });

  it('falls back to defaults when localStorage value is not an object', async () => {
    storage['cypcb-settings'] = JSON.stringify('just a string');

    const { getSettings, DEFAULT_SETTINGS } = await importSettings();
    expect(getSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it('exposes window.__settings debug surface', async () => {
    await importSettings();
    expect((globalThis as any).window.__settings).toBeDefined();
    expect((globalThis as any).window.__settings.units).toBe('mm');
  });

  it('get/set round-trip for layerColors', async () => {
    const { getPreference, setPreference } = await importSettings();

    const newColors = {
      topCopper: '#FF0000',
      bottomCopper: '#00FF00',
      silkscreen: '#0000FF',
      via: '#AAAAAA',
      drill: '#000000',
      innerCopper: ['#112233', '#445566'],
    };
    setPreference('layerColors', newColors);
    expect(getPreference('layerColors')).toEqual(newColors);
  });

  it('resetSettings restores defaults and notifies', async () => {
    const { setPreference, resetSettings, getSettings, subscribe, DEFAULT_SETTINGS } = await importSettings();

    setPreference('units', 'mil');
    const listener = vi.fn();
    subscribe(listener);

    resetSettings();

    expect(getSettings()).toEqual(DEFAULT_SETTINGS);
    expect(listener).toHaveBeenCalledTimes(1);
  });
});
