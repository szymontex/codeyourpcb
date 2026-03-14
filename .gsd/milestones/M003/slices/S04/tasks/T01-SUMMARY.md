---
id: T01
parent: S04
milestone: M003
provides:
  - Typed AppSettings interface covering all preference keys
  - Settings persistence module with get/set/subscribe/reset API
  - Unit formatting (formatDimension) and parsing (parseUserDimension) for mm/mil/µm
  - DEFAULT_SETTINGS matching current hardcoded behavior
  - window.__settings debug surface for E2E inspection
key_files:
  - viewer/src/settings.ts
  - viewer/src/units.ts
  - viewer/src/__tests__/settings.test.ts
  - viewer/src/__tests__/units.test.ts
key_decisions:
  - Mil precision set to 4 decimal places (0.0001mil ≈ 2.54nm) to preserve round-trip fidelity for exact multiples of NM_PER_MIL
  - Settings stored as single JSON blob under 'cypcb-settings' localStorage key, with partial-merge on load so new keys get defaults automatically
  - Flat module-level state (not class) to match simpler import ergonomics — subscribe pattern identical to ThemeManager
patterns_established:
  - Settings subscribe pattern: subscribe(listener) returns unsubscribe fn, listener receives full AppSettings snapshot
  - Unit formatting pattern: formatDimension(nm, unit) for display, parseUserDimension(str) for input — all internal values in nanometers
  - Module-level vi.resetModules() + dynamic import() for testing modules with top-level side effects in node env
observability_surfaces:
  - window.__settings exposes current settings snapshot (updated on every setPreference call)
  - console.warn on localStorage parse failure with fallback to defaults
duration: 25min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Settings module, unit system, and Preferences-driven RenderConfig

**Built typed settings persistence layer and unit formatting/parsing system — the foundation for all S04 UI controls.**

## What Happened

Created two new modules:

**settings.ts** — AppSettings interface with typed keys for theme, units, gridVisualSpacing, gridSnapSpacing, gridVisible, ratsnestVisible, netLabelsVisible, and layerColors. API: `getPreference(key)`, `setPreference(key, value)`, `subscribe(listener)`, `getSettings()`, `resetSettings()`. Uses localStorage with single JSON key, deep-copies layerColors to prevent mutation, and exposes `window.__settings` for E2E. Defaults: mm units, 1mm visual grid (1_000_000nm), 50mil snap grid (1_270_000nm), all overlays visible, standard RenderConfig colors.

**units.ts** — `DisplayUnit` type (`'mm' | 'mil' | 'µm'`), `formatDimension(nm, unit)` with trailing-zero stripping, `parseUserDimension(input)` accepting mm/mil/µm/um suffixes (case-insensitive, whitespace-tolerant). Conversion constants exported for direct use.

Both modules have comprehensive test suites (12 settings tests, 20 units tests).

## Verification

- `cd viewer && npx vitest run src/__tests__/settings.test.ts` — 12/12 passed
- `cd viewer && npx vitest run src/__tests__/units.test.ts` — 20/20 passed
- `cd viewer && npx vitest run` — 109/109 passed (all existing tests unaffected)
- `cd viewer && npx tsc --noEmit` — zero errors

Slice-level verification (partial — T01 is first of 3 tasks):
- ✅ `cd viewer && npx vitest run` — all unit tests pass
- ⬜ `cd viewer && npx playwright test e2e/ui-architecture.spec.ts` — not yet created (T03)
- ⬜ `cd viewer && npx playwright test` — full E2E suite (T03)
- ✅ `npx tsc --noEmit` — zero errors

## Diagnostics

- `window.__settings` in browser console shows current settings snapshot
- localStorage key `cypcb-settings` can be inspected/edited directly
- Console warns `[settings] Failed to parse localStorage data` on corrupt data, falls back to defaults

## Deviations

- Mil format precision increased from 2 to 4 decimal places — 2 decimals caused round-trip precision loss for values not evenly divisible by 25_400nm. 4 decimals give sub-nm fidelity.
- `resetSettings()` added as utility for testing — not in original plan but needed for test isolation and useful for "reset to defaults" UI.

## Known Issues

None.

## Files Created/Modified

- `viewer/src/settings.ts` — NEW: typed settings module with localStorage persistence and change notification
- `viewer/src/units.ts` — NEW: unit formatting and parsing utilities with mm/mil/µm support
- `viewer/src/__tests__/settings.test.ts` — NEW: 12 unit tests covering defaults, persistence, subscribe/unsubscribe, edge cases
- `viewer/src/__tests__/units.test.ts` — NEW: 20 unit tests covering formatting, parsing, round-trips, edge cases
