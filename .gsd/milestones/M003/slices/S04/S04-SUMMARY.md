---
id: S04
parent: M003
milestone: M003
provides:
  - Typed AppSettings interface with get/set/subscribe/reset API and localStorage persistence
  - Unit formatting (formatDimension) and parsing (parseUserDimension) for mm/mil/µm
  - Clean toolbar with only essential tools (Editor, Undo/Redo, Fit SVG, View, 3D, Theme, Prefs, Coords, Open/Share, Route/Cancel/Auto-route)
  - View dropdown menu controlling Top layer, Bottom layer, Ratsnest, Grid visible, Net labels
  - Preferences modal with Display (theme, units), Grid (visual/snap spacing), Colors (5 layer pickers)
  - Grid visibility toggle independent of routing grid snap (fixes "grid toggle does nothing" bug)
  - Settings subscription driving RenderConfig mutation, routing state sync, and dirty flag
  - window.__settings debug surface for E2E inspection
  - 15 new E2E tests, 32 new unit tests, all 73 E2E + 109 unit tests passing
requires:
  - slice: S01
    provides: RenderConfig with layer color customization hooks
affects:
  - S05 (UI architecture, settings API, unit display system)
  - S06 (panel infrastructure, settings persistence layer)
key_files:
  - viewer/src/settings.ts
  - viewer/src/units.ts
  - viewer/index.html
  - viewer/src/main.ts
  - viewer/src/renderer.ts
  - viewer/e2e/ui-architecture.spec.ts
  - viewer/src/__tests__/settings.test.ts
  - viewer/src/__tests__/units.test.ts
key_decisions:
  - Grid visibility and grid snap are independent controls — View menu toggles visual grid, Preferences sets snap spacing
  - Settings persistence uses single localStorage key 'cypcb-settings' as JSON blob with partial-merge on load
  - View menu is a dropdown (not side panel) — single-click access to layer/grid/ratsnest
  - Mil format precision 4 decimal places for round-trip fidelity
  - Preferences modal uses inline event handlers per setPreference() call
  - Color pickers use 'input' event (not 'change') for live preview
patterns_established:
  - Settings subscribe pattern: subscribe(listener) returns unsubscribe fn, listener receives full AppSettings snapshot
  - Unit formatting pattern: formatDimension(nm, unit) for display, parseUserDimension(str) for input — all internal values in nanometers
  - View dropdown pattern: button toggles .hidden, click-outside and Escape close it
  - Preferences modal pattern: overlay with backdrop click + Escape + X close; populate from getSettings() on open
observability_surfaces:
  - window.__settings exposes live settings snapshot (updated on every setPreference call)
  - console.warn on localStorage parse failure with fallback to defaults
  - console.warn on invalid grid spacing or malformed color hex in Preferences
drill_down_paths:
  - .gsd/milestones/M003/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M003/slices/S04/tasks/T02-SUMMARY.md
  - .gsd/milestones/M003/slices/S04/tasks/T03-SUMMARY.md
duration: 3 tasks across 1 context window
verification_result: passed
completed_at: 2026-03-13
---

# S04: UI Architecture — Toolbar, View Menu & Settings

**Clean toolbar (6 controls removed), View dropdown with layer/grid/ratsnest toggles, Preferences modal with theme/units/grid/colors, unit system (mm/mil/µm) wired throughout, all settings persist to localStorage.**

## What Happened

Three tasks delivered the full UI architecture restructuring:

**T01** built the foundation — typed `AppSettings` interface with `getPreference(key)` / `setPreference(key, value)` / `subscribe(listener)` / `resetSettings()`. Persistence via single JSON key in localStorage with partial-merge on load (new keys get defaults automatically). Unit system: `formatDimension(nm, unit)` and `parseUserDimension(str)` supporting mm/mil/µm with trailing-zero stripping and case-insensitive parsing. 12 + 20 unit tests covering settings and units respectively.

**T02** restructured the visible UI — removed 6 toolbar controls (layer checkboxes, grid snap, labels, separators), added View dropdown button with the toggles relocated inside, added ⚙ Preferences button opening a centered modal with three sections (Display: theme + units, Grid: visual + snap spacing with parseUserDimension validation, Colors: 5 layer color pickers). Wired all controls through settings module. Added grid visibility flag to renderer (independent of routing grid snap — fixes "grid toggle does nothing" bug). Replaced ⊡ fit icon with inline SVG crosshair. Wired `formatDimension()` to coords display and trace tooltip.

**T03** wrote 15 E2E tests across 4 describe blocks (Toolbar Structure, View Menu, Preferences Modal, Persistence) and updated 4 existing test files for View-dropdown selectors. Full suite: 73/73 E2E, 109/109 unit tests.

## Verification

- `npx tsc --noEmit` — zero errors ✅
- `npx vitest run` — 109/109 unit tests (9 files) ✅
- `npx playwright test e2e/ui-architecture.spec.ts` — 15/15 passed ✅
- `npx playwright test` — 73/73 full suite passed ✅

## Requirements Advanced

- UI-04 (manual dark/light toggle) — theme cycle button in Preferences modal with single-click fix
- UI-05 (theme consistency) — Preferences modal and View dropdown styled with theme CSS variables

## Requirements Validated

- None newly validated (UI-04, UI-05 were already validated)

## New Requirements Surfaced

- None

## Requirements Invalidated or Re-scoped

- None

## Deviations

- routing.ts was not directly modified — grid snap spacing update handled via settings subscription in main.ts which mutates `routingState.gridSpacing`. Routing module already reads from its state.
- renderer-quality.spec.ts also needed updating (not in task plan) — its performance test toggled `#layer-top` directly.
- Persistence tests used `page.evaluate` to clear localStorage instead of `addInitScript` — the latter re-executed on reload and destroyed the settings under test.

## Known Limitations

- ThemeManager maintains its own `'theme'` localStorage key separate from settings module — by design, for FART (flash of incorrect theme) prevention, but means theme has two sync paths.
- Settings module is flat module-level state (not class) — sufficient for current scale but would need refactoring if multiple settings scopes were ever needed.

## Follow-ups

- S05 consumes settings API for project manager preferences
- S06 consumes panel infrastructure for JLCPCB search UI
- Layer color changes propagate to RenderConfig but 3D renderer doesn't consume settings yet (S06/S07 scope)

## Files Created/Modified

- `viewer/src/settings.ts` — NEW: typed settings with localStorage persistence and change notification
- `viewer/src/units.ts` — NEW: unit formatting/parsing for mm/mil/µm
- `viewer/src/__tests__/settings.test.ts` — NEW: 12 unit tests
- `viewer/src/__tests__/units.test.ts` — NEW: 20 unit tests
- `viewer/e2e/ui-architecture.spec.ts` — NEW: 15 E2E tests for View menu, Preferences, persistence
- `viewer/index.html` — Restructured toolbar, added View dropdown + Preferences modal + SVG fit icon + CSS
- `viewer/src/main.ts` — Settings wiring, View/Prefs handlers, formatDimension in coords, settings subscription
- `viewer/src/renderer.ts` — gridVisible/gridVisualSpacing/showNetLabels in RenderState, parameterized grid
- `viewer/e2e/app-load.spec.ts` — Updated toolbar checks for new structure
- `viewer/e2e/board-interaction.spec.ts` — Layer toggles open View menu first
- `viewer/e2e/renderer-quality.spec.ts` — Performance test opens View menu before layer toggle

## Forward Intelligence

### What the next slice should know
- `getPreference(key)` / `setPreference(key, value)` are the API for reading/writing any app setting. Import from `./settings.ts`.
- `formatDimension(nm, unit)` from `./units.ts` is how all user-facing dimensions should be displayed. Current unit is `getPreference('units')`.
- The settings subscribe pattern returns an unsubscribe function — store it if you need cleanup.
- View dropdown and Preferences modal patterns in `index.html` can be extended for new panels.

### What's fragile
- Settings subscription in main.ts is a growing switch/case on changed keys — if many more settings are added, extract to a dedicated sync module.
- Grid visual spacing and snap spacing are both stored in nanometers — passing raw user input without parseUserDimension will silently set wrong values.

### Authoritative diagnostics
- `window.__settings` in browser console — shows live settings snapshot, updated on every change. E2E tests use this as primary assertion surface.
- `localStorage.getItem('cypcb-settings')` — raw persisted JSON blob, inspectable for debugging persistence issues.

### What assumptions changed
- routing.ts didn't need direct modification — settings subscription in main.ts handles the wiring, which is simpler than originally planned.
