---
estimated_steps: 5
estimated_files: 4
---

# T01: Settings module, unit system, and Preferences-driven RenderConfig

**Slice:** S04 — UI Architecture — Toolbar, View Menu & Settings
**Milestone:** M003

## Description

Build the two foundational modules that everything else in S04 depends on: a typed settings persistence layer and a unit formatting system. The settings module follows the same subscribe-notify pattern as ThemeManager but generalizes it to all preferences. The unit system provides `formatDimension()` used by every dimension display site in the app. Both modules get comprehensive unit tests before any UI wiring begins.

## Steps

1. Create `viewer/src/settings.ts` — Define `AppSettings` interface covering all preference keys (theme, units, gridVisualSpacing, gridSnapSpacing, gridVisible, ratsnestVisible, netLabelsVisible, layerColors). Implement `getPreference(key)`, `setPreference(key, value)`, `subscribe(listener)`, `getSettings()`. Use localStorage with a single JSON key `'cypcb-settings'`. Provide `DEFAULT_SETTINGS` const matching current behavior. Expose `window.__settings` for E2E inspection.

2. Create `viewer/src/units.ts` — Define `DisplayUnit` type (`'mm' | 'mil' | 'µm'`). Implement `formatDimension(nm: number, unit: DisplayUnit): string` (e.g., 2_540_000 → "2.54mm" / "100.00mil" / "2540µm"). Implement `parseUserDimension(input: string): number | null` that accepts "2.54mm", "100mil", "2540µm" and returns nanometers. Export conversion constants.

3. Write `viewer/src/__tests__/settings.test.ts` — Test: defaults when localStorage empty, get/set round-trip, persistence to localStorage, subscribe notification fires on change, multiple subscribers, unsubscribe works, invalid localStorage data falls back to defaults, partial settings merge with defaults.

4. Write `viewer/src/__tests__/units.test.ts` — Test: formatDimension for each unit type (mm, mil, µm), edge cases (0, negative, very large), parseUserDimension round-trip, parseUserDimension with invalid input returns null, whitespace tolerance, case insensitivity for unit suffix.

5. Verify both test suites pass and TypeScript compiles clean.

## Must-Haves

- [ ] `AppSettings` interface with typed keys for all preferences
- [ ] `getPreference()` / `setPreference()` with localStorage persistence
- [ ] `subscribe()` returning unsubscribe function (ThemeManager pattern)
- [ ] `DEFAULT_SETTINGS` matching current behavior (mm units, 1mm grid visual, 50mil grid snap, all layers visible)
- [ ] `formatDimension(nm, unit)` producing correct formatted strings for mm/mil/µm
- [ ] `parseUserDimension(str)` returning nanometers or null
- [ ] `window.__settings` debug surface
- [ ] Unit tests covering both modules with edge cases

## Verification

- `cd viewer && npx vitest run src/__tests__/settings.test.ts` — all pass
- `cd viewer && npx vitest run src/__tests__/units.test.ts` — all pass
- `cd viewer && npx tsc --noEmit` — zero errors

## Inputs

- `viewer/src/theme/theme-manager.ts` — subscribe pattern to follow
- `viewer/src/render-config.ts` — RenderConfig interface (settings module must cover all configurable fields)
- `viewer/src/renderer.ts` line 211 — current grid spacing hardcoded to 1mm (default must match)
- `viewer/src/routing.ts` line 82 — current grid snap hardcoded to 50mil (default must match)

## Expected Output

- `viewer/src/settings.ts` — NEW: typed settings module with localStorage persistence and change notification
- `viewer/src/units.ts` — NEW: unit formatting and parsing utilities
- `viewer/src/__tests__/settings.test.ts` — NEW: 8+ unit tests
- `viewer/src/__tests__/units.test.ts` — NEW: 8+ unit tests
