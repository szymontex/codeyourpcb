# S04: UI Architecture — Toolbar, View Menu & Settings

**Goal:** Toolbar contains only essential tools; View menu/panel controls layers, grid, ratsnest, net labels; Preferences panel sets theme, units, grid spacing, layer colors — all persisted to localStorage. Unit display (mm/mils/µm) works throughout the UI.
**Demo:** User opens View menu to toggle layer visibility and grid, opens Preferences to switch units from mm to mils, sees all coordinate displays update. Toolbar is clean — no layer checkboxes or grid snap cluttering the bar.

## Must-Haves

- Settings module with typed `getPreference(key)` / `setPreference(key, value)` API, localStorage persistence, and change notification (subscribe pattern)
- Unit system: `formatDimension(nm, unit)` → `"2.54mm"` / `"100mil"` / `"2540µm"` used at all dimension display sites
- Toolbar cleaned to: Editor, Undo/Redo, Fit (SVG icon), 3D, Theme, Open/Share, Route/Cancel/Auto-route
- View menu (dropdown) with: Top layer, Bottom layer, Ratsnest, Grid visibility, Net labels — toggling controls that feed into RenderState
- Grid visibility toggle separate from grid snap (fixes the "grid toggle does nothing" bug)
- Preferences modal with sections: Display (theme, units), Grid (visual spacing, snap spacing), Colors (layer color overrides) — writes to settings → propagates to RenderConfig
- All settings have sensible defaults matching current behavior (empty localStorage = same UX as today)
- Existing E2E tests updated for moved selectors; new E2E tests for View menu, Preferences, unit switching

## Proof Level

- This slice proves: operational (settings persist, unit display propagates, View menu toggles work)
- Real runtime required: yes (Canvas rendering, localStorage, DOM interaction)
- Human/UAT required: no (E2E covers all flows)

## Verification

- `cd viewer && npx vitest run` — all unit tests pass (settings, units, existing)
- `cd viewer && npx playwright test e2e/ui-architecture.spec.ts` — new E2E tests for View menu, Preferences, unit switching
- `cd viewer && npx playwright test` — full suite passes (existing tests updated for moved elements)
- `npx tsc --noEmit` — zero TypeScript errors

## Observability / Diagnostics

- Runtime signals: settings change events via subscribe pattern (same as ThemeManager)
- Inspection surfaces: `window.__settings` debug surface exposing current settings snapshot for E2E; `localStorage` directly inspectable
- Failure visibility: console warning if settings deserialization fails (falls back to defaults)

## Integration Closure

- Upstream surfaces consumed: `RenderConfig` from `render-config.ts` (S01), `ThemeManager` from `theme-manager.ts`, `drawGrid()` in `renderer.ts`, `gridSpacing` in `routing.ts`
- New wiring introduced: settings.ts → RenderConfig mutation → `dirty = true` trigger; settings.ts → routing gridSpacing; formatDimension() at all dimension display sites
- What remains before milestone is truly usable end-to-end: S05 (project manager), S06 (JLCPCB), S07 (polish)

## Tasks

- [x] **T01: Settings module, unit system, and Preferences-driven RenderConfig** `est:1.5h`
  - Why: Foundation for all UI controls — settings persistence, unit formatting, RenderConfig mutation from preferences. Nothing else can wire up without this.
  - Files: `viewer/src/settings.ts`, `viewer/src/units.ts`, `viewer/src/__tests__/settings.test.ts`, `viewer/src/__tests__/units.test.ts`
  - Do: Build typed settings module with localStorage persistence, defaults, and subscribe pattern. Build unit formatting with `formatDimension(nm, unit)` and `parseUserDimension(str)`. Write comprehensive unit tests for both modules. Expose `window.__settings` for E2E.
  - Verify: `cd viewer && npx vitest run src/__tests__/settings.test.ts src/__tests__/units.test.ts` — all pass
  - Done when: settings module can get/set/persist all preference types, unit formatter produces correct output for mm/mils/µm, and change subscribers fire correctly

- [x] **T02: Toolbar cleanup, View menu dropdown, Preferences modal, and fit icon fix** `est:2h`
  - Why: The visible UI restructuring — moves layer/grid/ratsnest to View menu, builds Preferences modal, cleans toolbar. This is where the user sees the change.
  - Files: `viewer/index.html`, `viewer/src/main.ts`, `viewer/src/renderer.ts`, `viewer/src/routing.ts`
  - Do: Restructure toolbar HTML (remove layer checkboxes, grid snap, add View button + dropdown). Build View dropdown with toggle controls for layers, grid visibility, ratsnest, net labels. Build Preferences modal with theme, units, grid spacing, layer colors. Wire all controls to settings module. Add grid visibility flag to renderer (separate from grid snap). Replace ⊡ fit icon with SVG. Wire `formatDimension()` to all dimension display sites (coords, trace tooltip). Update existing E2E test selectors for moved elements.
  - Verify: `npx tsc --noEmit` passes; app loads with clean toolbar; View menu opens and toggles work; Preferences modal opens and persists
  - Done when: toolbar has only essential tools, View menu controls layer/grid/ratsnest visibility, Preferences sets theme/units/grid/colors, all settings persist across reload

- [x] **T03: E2E tests and full-suite verification** `est:1h`
  - Why: Proves the slice works end-to-end — View menu toggles, Preferences persistence, unit switching, grid visibility vs snap separation. Also ensures no regressions in existing tests.
  - Files: `viewer/e2e/ui-architecture.spec.ts`, `viewer/e2e/app-load.spec.ts`, `viewer/e2e/board-interaction.spec.ts`, `viewer/e2e/reliability.spec.ts`
  - Do: Write new E2E tests covering: View menu open/close, layer toggle via View menu, grid visibility toggle, Preferences modal open/close, unit switching with coordinate display verification, settings persist across page reload. Update existing E2E tests that reference moved selectors (`#layer-top`, `#layer-bottom`, `#layer-ratsnest`, `#grid-snap`).
  - Verify: `cd viewer && npx playwright test` — full suite passes
  - Done when: all E2E tests pass including new ui-architecture.spec.ts; no regressions in existing tests

## Files Likely Touched

- `viewer/src/settings.ts` — NEW
- `viewer/src/units.ts` — NEW
- `viewer/src/__tests__/settings.test.ts` — NEW
- `viewer/src/__tests__/units.test.ts` — NEW
- `viewer/e2e/ui-architecture.spec.ts` — NEW
- `viewer/index.html` — toolbar restructure, View menu, Preferences modal
- `viewer/src/main.ts` — settings wiring, formatDimension usage, View/Prefs event handlers
- `viewer/src/renderer.ts` — grid visibility flag, grid spacing from settings
- `viewer/src/routing.ts` — grid snap spacing from settings
- `viewer/e2e/app-load.spec.ts` — selector updates
- `viewer/e2e/board-interaction.spec.ts` — selector updates
- `viewer/e2e/reliability.spec.ts` — selector updates
