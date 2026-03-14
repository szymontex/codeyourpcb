---
id: T02
parent: S04
milestone: M003
provides:
  - Clean toolbar with only essential tools (Editor, Undo/Redo, Fit SVG, View, 3D, Theme, Prefs, Coords, Open/Share, Route/Cancel/Auto-route)
  - View dropdown menu controlling Top layer, Bottom layer, Ratsnest, Grid visible, Net labels
  - Preferences modal with Display (theme cycle, unit select), Grid (visual/snap spacing), Colors (5 layer color pickers)
  - Grid visibility toggle separate from routing grid snap (fixes "grid toggle does nothing" bug)
  - formatDimension() wired into coords display and trace tooltip
  - Settings subscription propagating all changes to RenderConfig and routing state
key_files:
  - viewer/index.html
  - viewer/src/main.ts
  - viewer/src/renderer.ts
key_decisions:
  - View menu controls (layer toggles, grid visible, net labels) live inside a dropdown positioned below the View button, not in a side panel — keeps the toolbar clean while maintaining single-click access
  - Grid visibility is a separate boolean from grid snap, passed through RenderState to renderer — the old grid-snap checkbox only toggled routing snap, never visual grid
  - Preferences modal uses inline event handlers mapping to setPreference() rather than a generic data-pref attribute walker — simpler, more explicit, and avoids runtime string-to-key coercion
  - Color pickers use 'input' event (not 'change') for live preview while picking colors
patterns_established:
  - View dropdown pattern: button toggles .hidden class, document click-outside and Escape listeners close it
  - Preferences modal pattern: overlay with backdrop click + Escape + X button close; populate from getSettings() on open, write via setPreference() on each input change
  - Settings subscription drives RenderConfig mutation and dirty flag — single source of truth for all preference-dependent render state
observability_surfaces:
  - "window.__settings" reflects live settings state after any View/Prefs change
  - Console warns "[prefs] Invalid grid visual spacing, reverting" on bad grid input
  - Console warns "[prefs] Invalid color value" on malformed hex
duration: 45min
verification_result: passed
blocker_discovered: false
---

# T02: Toolbar cleanup, View menu dropdown, Preferences modal, and fit icon fix

**Restructured toolbar (removed 6 controls), added View dropdown with 5 toggles, built Preferences modal with theme/units/grid/colors, wired formatDimension() to all dimension display sites, replaced ⊡ with SVG fit icon.**

## What Happened

Removed layer checkboxes (Top, Bottom, Ratsnest), grid snap checkbox, associated labels and separators from the toolbar. Added a View dropdown button with the layer toggles relocated inside it plus new Grid visible and Net labels toggles. Added a ⚙ Preferences button that opens a centered modal with three sections: Display (theme cycle button, unit select with mm/mil/µm), Grid (visual spacing and snap spacing text inputs with parseUserDimension validation), and Colors (5 color pickers for layer colors).

All controls wire through the T01 settings module — `setPreference()` on change, `getPreference()` on init. A settings subscription syncs all changes to RenderConfig layer colors, routing grid spacing, grid visibility, net labels visibility, and ratsnest visibility, then sets `dirty = true`.

The renderer now accepts `gridVisible`, `gridVisualSpacing`, and `showNetLabels` in RenderState. When `gridVisible` is false, `drawGrid()` is skipped entirely — independent of routing grid snap. The `drawGrid()` function now takes grid spacing as a parameter instead of hardcoding 1mm.

Coordinate display uses `formatDimension(worldX, unit)` instead of hardcoded mm division. Trace tooltip (`drawNetLabel`) uses `formatDimension(trace.width, unit)` instead of hardcoded mm.

The fit button ⊡ character was replaced with an inline SVG showing a crosshair/fit icon (rect with 4 extending lines).

## Verification

- `npx tsc --noEmit` — zero errors ✅
- `npx vitest run` — 109/109 tests pass (9 test files) ✅
- Browser verification skipped (no X display in this environment)

### Slice-level verification status (T02 is intermediate):
- `cd viewer && npx vitest run` — ✅ all pass
- `cd viewer && npx playwright test e2e/ui-architecture.spec.ts` — not yet created (T03)
- `cd viewer && npx playwright test` — expected some failures on `#layer-top`/`#layer-bottom` visibility assertions since they're now inside hidden dropdown (T03 will fix)
- `npx tsc --noEmit` — ✅ zero errors

## Diagnostics

- `window.__settings` in browser console shows full settings snapshot including any changes made via View menu or Preferences modal
- `localStorage.getItem('cypcb-settings')` shows persisted JSON blob
- Console warns on invalid grid spacing input or malformed color hex values in Preferences

## Deviations

- routing.ts was not directly modified — grid snap spacing is updated via settings subscription in main.ts which mutates `routingState.gridSpacing` and syncs to `interactionState.routing`. The routing module already reads from its state object, so no source change needed.
- Used 'input' event for color pickers (live preview while picking) instead of 'change' event.

## Known Issues

- E2E tests in `app-load.spec.ts` and `board-interaction.spec.ts` reference `#layer-top` / `#layer-bottom` with `toBeVisible()` — these elements now live inside a hidden dropdown and won't pass visibility checks. T03 will update these tests.

## Files Created/Modified

- `viewer/index.html` — Restructured toolbar (removed 6 controls, added View dropdown + Preferences modal + SVG fit icon), added CSS for dropdown, modal, and prefs button
- `viewer/src/main.ts` — Added settings/units imports, View menu wiring, Preferences modal handlers, formatDimension in coords display, settings subscription for RenderConfig sync, grid visibility/net labels state
- `viewer/src/renderer.ts` — Added gridVisible/gridVisualSpacing/showNetLabels to RenderState, conditional grid drawing, parameterized grid spacing, formatDimension in trace tooltip
