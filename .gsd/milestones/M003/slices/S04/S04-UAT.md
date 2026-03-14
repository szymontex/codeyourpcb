# S04: UI Architecture — Toolbar, View Menu & Settings — UAT

**Milestone:** M003
**Written:** 2026-03-13

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: All flows covered by 15 dedicated E2E tests + 32 unit tests. View menu toggles, Preferences persistence, unit switching, and grid visibility vs snap independence are all verified programmatically.

## Preconditions

- `cd viewer && npm run dev` running (or `npx vite build` for production check)
- Browser open to the app URL

## Smoke Test

Open the app. Toolbar should show only essential buttons (no layer checkboxes visible). Click "View" → dropdown appears with layer toggles. Click ⚙ → Preferences modal opens.

## Test Cases

### 1. Toolbar is clean

1. Load the app
2. Inspect the toolbar area
3. **Expected:** Only these buttons visible: Editor toggle, Undo, Redo, Fit (SVG icon), View, 3D, Theme, ⚙, coordinates display, Open/Share, Route/Cancel/Auto-route. No layer checkboxes, no grid snap checkbox in the toolbar bar.

### 2. View menu opens and controls layers

1. Click "View" button in toolbar
2. View dropdown appears with checkboxes: Top layer, Bottom layer, Ratsnest, Grid visible, Net labels
3. Uncheck "Top layer"
4. **Expected:** Top copper layer disappears from the board view. Checking it again restores it.

### 3. View menu closes properly

1. Open View menu
2. Press Escape
3. **Expected:** Dropdown closes
4. Open View menu again
5. Click anywhere outside the dropdown
6. **Expected:** Dropdown closes

### 4. Preferences modal — unit switching

1. Click ⚙ button
2. Preferences modal opens with Display, Grid, Colors sections
3. Change units from "mm" to "mil"
4. Close Preferences (X button, Escape, or backdrop click)
5. **Expected:** Coordinate display in toolbar now shows values in mils (e.g., "393.7mil" instead of "10mm")

### 5. Preferences modal — grid spacing

1. Open Preferences
2. Change "Visual grid spacing" to "2mm"
3. Close Preferences
4. **Expected:** Grid lines on the board are spaced 2mm apart (visually wider)

### 6. Preferences modal — layer colors

1. Open Preferences
2. In Colors section, change Top Copper color to green
3. Close Preferences
4. **Expected:** Top copper layer renders in green instead of red

### 7. Settings persist across reload

1. Open Preferences, switch units to µm, close
2. Reload the page (F5)
3. **Expected:** Units still show µm. Open Preferences — µm is selected in the dropdown.

### 8. Grid visibility vs grid snap are independent

1. Open View menu, uncheck "Grid visible"
2. **Expected:** Grid lines disappear from the board
3. Start routing a trace
4. **Expected:** Trace still snaps to grid positions (snap is still active even though grid is invisible)

## Edge Cases

### Corrupt localStorage

1. In browser console: `localStorage.setItem('cypcb-settings', 'garbage{{')`
2. Reload the page
3. **Expected:** Console warns about parse failure. App loads with default settings (mm units, all layers visible, standard colors).

### Empty localStorage

1. In browser console: `localStorage.removeItem('cypcb-settings')`
2. Reload the page
3. **Expected:** App loads with default settings identical to fresh install.

## Failure Signals

- Layer checkboxes visible directly in toolbar (not in View dropdown) → T02 regression
- Coordinate display still shows hardcoded "mm" after switching to mil → formatDimension not wired
- Settings lost on reload → localStorage persistence broken
- Grid toggle does nothing visible → grid visibility flag not wired to renderer
- Preferences modal doesn't close on Escape/backdrop → event handler missing
- console errors on settings read/write → settings module broken

## Requirements Proved By This UAT

- UI-04 — Theme toggle works via single-click (not double-click) in Preferences
- UI-05 — Theme applies to Preferences modal and View dropdown consistently

## Not Proven By This UAT

- 3D renderer consuming layer color changes from settings (deferred to S06/S07)
- Project manager integration with settings (S05 scope)
- JLCPCB panel using settings panel infrastructure (S06 scope)

## Notes for Tester

- The `window.__settings` debug surface shows live settings — useful for verifying programmatically
- ThemeManager has its own localStorage key ('theme') separate from settings ('cypcb-settings') — this is intentional for FART prevention
- The fit button is now an SVG icon (crosshair with extending lines) instead of the old ⊡ character
