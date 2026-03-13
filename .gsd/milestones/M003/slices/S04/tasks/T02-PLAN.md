---
estimated_steps: 7
estimated_files: 5
---

# T02: Toolbar cleanup, View menu dropdown, Preferences modal, and fit icon fix

**Slice:** S04 — UI Architecture — Toolbar, View Menu & Settings
**Milestone:** M003

## Description

The visible UI restructuring. Remove layer checkboxes, grid snap, and ratsnest toggle from the toolbar. Add a View dropdown menu and Preferences modal. Wire all controls to the settings module from T01. Replace the ⊡ fit icon with an SVG. Wire `formatDimension()` into coordinate display and trace tooltip. Add grid visibility as a separate control from grid snap. This is the highest-touch task — modifies index.html toolbar structure and main.ts wiring.

## Steps

1. Restructure toolbar in `viewer/index.html` — Remove: Layers label + #layer-top + #layer-bottom checkboxes, #layer-ratsnest checkbox, #grid-snap checkbox, associated separators. Add: `<button id="view-menu-btn">View</button>` and `<div id="view-menu-dropdown" class="hidden">` with checkbox controls for Top layer, Bottom layer, Ratsnest, Grid visible, Net labels. Add `<button id="prefs-btn">⚙</button>`. Replace ⊡ in #fit-btn with inline SVG (crosshair/fit icon). Keep stable IDs for moved controls inside the dropdown (`#layer-top`, `#layer-bottom`, `#layer-ratsnest` stay but live inside View dropdown now).

2. Add CSS for View dropdown and Preferences modal — Dropdown: positioned below View button, z-index above canvas but below modals, themed with CSS variables, click-outside-closes. Preferences modal: centered overlay with backdrop, sections for Display/Grid/Colors, form controls styled with existing CSS variables. Respect existing `@media` breakpoints.

3. Build Preferences modal HTML in `viewer/index.html` — Sections: Display (theme cycle button, unit select dropdown with mm/mil/µm), Grid (visual spacing input, snap spacing input, grid visible default), Colors (5 color pickers for layerColors: topCopper, bottomCopper, silkscreen, via, drill). All inputs get `data-pref="key"` attributes for generic wiring.

4. Wire View menu in `viewer/src/main.ts` — View button click toggles dropdown visibility. Each checkbox in dropdown reads from settings on init and writes to settings on change. Layer toggles update `layerVisibility` and set `dirty = true`. Grid visibility toggle sets a new `gridVisible` flag (separate from routing grid snap). Ratsnest toggle updates `showRatsnest` flag. Net labels toggle updates `showNetLabels` flag. Click outside dropdown closes it. Keyboard: Escape closes dropdown.

5. Wire Preferences modal in `viewer/src/main.ts` — Prefs button opens modal. On open, populate all inputs from current settings. On change, call `setPreference()`. Theme section: reuse existing theme cycle logic. Units section: `<select>` with mm/mil/µm, writes to settings. Grid spacing inputs: parse with `parseUserDimension()`, validate, write to settings. Color pickers: `<input type="color">`, write hex to settings. Close via X button or Escape. Settings changes propagate to RenderConfig (mutate renderConfig fields, set `dirty = true`). Grid snap spacing change updates routing state.

6. Wire `formatDimension()` to dimension display sites — Replace hardcoded `mm` in coords display (main.ts ~line 940) with `formatDimension()`. Replace hardcoded `mm` in trace width tooltip (renderer.ts ~line 507). Subscribe to unit setting change to trigger re-render.

7. Add grid visibility flag to renderer — `drawGrid()` in renderer.ts accepts a `gridVisible` boolean parameter. When false, skip grid drawing entirely. This is independent of routing grid snap. Wire the flag from View menu toggle through to render call.

## Must-Haves

- [ ] Toolbar contains only: Editor, Undo/Redo, Fit (SVG), View, 3D, Theme, Prefs, Coords, Open/Share, Route/Cancel/Auto-route
- [ ] View dropdown has: Top layer, Bottom layer, Ratsnest, Grid visible, Net labels — each toggling correctly
- [ ] Grid visibility toggle is separate from grid snap (fixes "grid toggle does nothing" bug)
- [ ] Preferences modal has: Theme, Units (mm/mil/µm), Grid visual spacing, Grid snap spacing, Layer colors
- [ ] All preference changes persist to localStorage via settings module
- [ ] Coords display and trace tooltip use `formatDimension()` with selected unit
- [ ] Fit icon replaced with readable SVG
- [ ] View dropdown closes on click-outside and Escape
- [ ] Preferences modal closes on X button and Escape
- [ ] TypeScript compiles clean

## Verification

- `cd viewer && npx tsc --noEmit` — zero errors
- Manual check: `cd viewer && npm run dev` — toolbar is clean, View menu opens with layer toggles, Preferences modal opens with unit selector, changing unit updates coords display, settings persist on reload

## Observability Impact

- Signals added/changed: `window.__settings` reflects live settings state after any View/Prefs change
- How a future agent inspects this: `window.__settings.getSettings()` in browser console or E2E
- Failure state exposed: console warning if color parse fails in preferences, settings fallback to defaults on corrupt localStorage

## Inputs

- `viewer/src/settings.ts` — T01 settings module (getPreference, setPreference, subscribe, DEFAULT_SETTINGS)
- `viewer/src/units.ts` — T01 formatDimension, parseUserDimension, DisplayUnit
- `viewer/src/render-config.ts` — RenderConfig interface with layerColors, fontConfig, lodThresholds
- `viewer/src/theme/theme-manager.ts` — existing theme cycle logic to reuse in Preferences

## Expected Output

- `viewer/index.html` — MODIFIED: restructured toolbar, View dropdown, Preferences modal, fit SVG icon
- `viewer/src/main.ts` — MODIFIED: View menu handlers, Preferences modal handlers, formatDimension wiring, settings subscription, grid visibility flag
- `viewer/src/renderer.ts` — MODIFIED: drawGrid accepts gridVisible flag, grid spacing from parameter, trace tooltip uses formatDimension
- `viewer/src/routing.ts` — MODIFIED: gridSpacing reads from settings (or accepts parameter)
