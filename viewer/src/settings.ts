/**
 * Application settings persistence and change notification.
 *
 * Follows the same subscribe-notify pattern as ThemeManager but generalizes
 * to all user preferences. Stores a single JSON blob in localStorage under
 * the key 'cypcb-settings'. Missing or corrupt data falls back to defaults.
 *
 * Debug surface: `window.__settings` exposes the current snapshot for E2E.
 */

import type { DisplayUnit } from './units';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface RecentFileEntry {
  name: string;
  timestamp: number;
  thumbnail: string | null;
}

export interface LayerColors {
  topCopper: string;
  bottomCopper: string;
  silkscreen: string;
  via: string;
  drill: string;
}

export interface AppSettings {
  /** Theme preference: light, dark, or auto (follow OS) */
  theme: 'light' | 'dark' | 'auto';
  /** Display unit for dimensions */
  units: DisplayUnit;
  /** Visual grid spacing in nanometers */
  gridVisualSpacing: number;
  /** Snap grid spacing in nanometers */
  gridSnapSpacing: number;
  /** Whether the grid overlay is visible */
  gridVisible: boolean;
  /** Whether the ratsnest overlay is visible */
  ratsnestVisible: boolean;
  /** Whether net labels are visible */
  netLabelsVisible: boolean;
  /** Layer color overrides */
  layerColors: LayerColors;
  /** Recently opened files (newest first, max 10) */
  recentFiles: RecentFileEntry[];
}

export type SettingsKey = keyof AppSettings;
export type SettingsListener = (settings: AppSettings) => void;

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

/**
 * Default settings matching current hardcoded behavior:
 * - mm units
 * - 1mm visual grid (1_000_000 nm)
 * - 50mil snap grid (1_270_000 nm)
 * - all overlays visible
 * - standard RenderConfig layer colors
 */
export const DEFAULT_SETTINGS: Readonly<AppSettings> = {
  theme: 'auto',
  units: 'mm',
  gridVisualSpacing: 1_000_000,
  gridSnapSpacing: 1_270_000,
  gridVisible: true,
  ratsnestVisible: true,
  netLabelsVisible: true,
  layerColors: {
    topCopper: '#C83434',
    bottomCopper: '#3434C8',
    silkscreen: '#C8C800',
    via: '#808080',
    drill: '#FFFFFF',
  },
  recentFiles: [],
};

// ---------------------------------------------------------------------------
// Storage key
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'cypcb-settings';

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

function loadFromStorage(): AppSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_SETTINGS, layerColors: { ...DEFAULT_SETTINGS.layerColors } };

    const parsed = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) {
      console.warn('[settings] Invalid localStorage data, falling back to defaults');
      return { ...DEFAULT_SETTINGS, layerColors: { ...DEFAULT_SETTINGS.layerColors } };
    }

    // Merge parsed values over defaults so missing keys get filled in
    return {
      ...DEFAULT_SETTINGS,
      ...parsed,
      layerColors: {
        ...DEFAULT_SETTINGS.layerColors,
        ...(parsed.layerColors ?? {}),
      },
      recentFiles: Array.isArray(parsed.recentFiles) ? parsed.recentFiles : [],
    };
  } catch (e) {
    console.warn('[settings] Failed to parse localStorage data, falling back to defaults', e);
    return { ...DEFAULT_SETTINGS, layerColors: { ...DEFAULT_SETTINGS.layerColors } };
  }
}

function saveToStorage(settings: AppSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Storage full or unavailable — silently degrade
  }
}

// ---------------------------------------------------------------------------
// Exported API
// ---------------------------------------------------------------------------

let current: AppSettings = loadFromStorage();
const listeners: Set<SettingsListener> = new Set();

function notifyListeners(): void {
  const snapshot = getSettings();
  listeners.forEach((fn) => fn(snapshot));
}

function exposeDebugSurface(): void {
  if (typeof window !== 'undefined') {
    (window as any).__settings = getSettings();
  }
}

/**
 * Get the full current settings snapshot (deep copy).
 */
export function getSettings(): AppSettings {
  return {
    ...current,
    layerColors: { ...current.layerColors },
  };
}

/**
 * Get a single preference value.
 */
export function getPreference<K extends SettingsKey>(key: K): AppSettings[K] {
  const val = current[key];
  // Deep-copy layerColors to prevent external mutation
  if (key === 'layerColors' && typeof val === 'object') {
    return { ...(val as any) } as AppSettings[K];
  }
  // Deep-copy recentFiles array
  if (key === 'recentFiles' && Array.isArray(val)) {
    return val.map((e: any) => ({ ...e })) as unknown as AppSettings[K];
  }
  return val;
}

/**
 * Set a single preference value, persist to localStorage, and notify listeners.
 */
export function setPreference<K extends SettingsKey>(key: K, value: AppSettings[K]): void {
  // Deep-copy layerColors on write
  if (key === 'layerColors' && typeof value === 'object') {
    current[key] = { ...(value as any) } as AppSettings[K];
  } else {
    current[key] = value;
  }
  saveToStorage(current);
  exposeDebugSurface();
  notifyListeners();
}

/**
 * Subscribe to any settings change.
 * @returns Unsubscribe function.
 */
export function subscribe(listener: SettingsListener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Reset all settings to defaults (useful for testing).
 */
export function resetSettings(): void {
  current = { ...DEFAULT_SETTINGS, layerColors: { ...DEFAULT_SETTINGS.layerColors } };
  saveToStorage(current);
  exposeDebugSurface();
  notifyListeners();
}

// ---------------------------------------------------------------------------
// Initialize debug surface
// ---------------------------------------------------------------------------

exposeDebugSurface();
