# S06: JLCPCB Integration & 3D Models — UAT

**Milestone:** M003
**Written:** 2026-03-14

## UAT Type

- UAT mode: mixed (artifact-driven for API/parser logic, live-runtime for search panel UX)
- Why this mode is sufficient: Core parsing logic is pure-function tested via vitest. UI flow and API integration verified via Playwright E2E with route interception. Live manual testing validates visual polish and real API behavior.

## Preconditions

- Dev server running: `cd viewer && npx vite` (default port 5173)
- Browser open to `http://localhost:5173`
- Internet connection available (for real JLCPCB API calls in manual tests)
- All unit tests passing: `npx vitest run` shows 127 pass
- All E2E tests passing: `npx playwright test` shows 93 pass

## Smoke Test

Click the 🔍 toolbar button → search panel appears on the right side → type "0805 10k" → results appear with LCSC part numbers, manufacturers, packages, prices, and stock counts.

## Test Cases

### 1. Search Panel Toggle

1. Load the app at `http://localhost:5173`
2. Locate the 🔍 button in the toolbar (after ⚙ Prefs button)
3. Click the 🔍 button
4. **Expected:** Search panel slides in from the right (360px wide), search input is focused
5. Click the 🔍 button again
6. **Expected:** Panel closes, button loses active styling
7. Press Ctrl+J
8. **Expected:** Panel reopens
9. Press Escape
10. **Expected:** Panel closes

### 2. Search Results with Metadata

1. Open the search panel (🔍 button)
2. Type "0805 10k" in the search input
3. Wait ~500ms for debounced search to complete
4. **Expected:** Results list shows multiple resistor entries, each displaying:
   - LCSC part number (e.g., C17414) in accent color
   - Package badge (e.g., "0805")
   - Manufacturer name
   - Component attributes (e.g., "Resistance: 10kΩ")
   - Price (4 decimal places, e.g., "$0.0017")
   - Stock count
   - "Datasheet" link (clickable, opens in new tab)

### 3. Search Result Datasheet Link

1. With search results visible from test 2
2. Click the "Datasheet" link on any result
3. **Expected:** Datasheet URL opens in a new browser tab. The result row is NOT selected (no component fetch triggered).

### 4. Empty Search Results

1. Open the search panel
2. Type a nonsense query like "zzzzxqwerty99999"
3. **Expected:** After debounce, panel shows "No results found" status message

### 5. Search Debounce Behavior

1. Open the search panel
2. Type "capacitor" quickly (all characters within 200ms)
3. Open browser DevTools Network tab, filter to `jlcsearch`
4. **Expected:** Only ONE request to `jlcsearch.tscircuit.com` is made (not one per keystroke)

### 6. Component Click with 3D View Active

1. Load a board file that has components (e.g., blink.cypcb or a template)
2. Switch to 3D view (click "3D" button or press "3" key)
3. Open search panel and search "0805 10k"
4. Click on a result row (not the datasheet link)
5. Open browser console (DevTools)
6. **Expected:** Console shows `[JLCPCB] Selected: CXXXXX (manufacturer)`. If EasyEDA API responds (may CORS-fail from localhost), either `[3D] OBJ loaded for CXXXXX` or `[JLCPCB] No 3D model available for CXXXXX` appears. No crashes.

### 7. Component Click with 2D View Active

1. Ensure 2D view is active (not 3D)
2. Open search panel and search "0805 10k"
3. Click on a result row
4. **Expected:** Console shows the selection log but NO 3D model fetch is attempted (no EasyEDA API calls in Network tab). Panel stays open.

### 8. Overlay Conflict — Preferences

1. Open the search panel (🔍 button)
2. Click the ⚙ Prefs button to open Preferences modal
3. **Expected:** Search panel closes automatically. Preferences modal is visible. No stacking of overlays.

### 9. Overlay Conflict — Project Manager

1. Open the search panel
2. Trigger project manager (if available via Ctrl+N or new file action)
3. **Expected:** Search panel closes when project manager opens

### 10. Theme Compatibility

1. Open Preferences, switch to Light theme
2. Open search panel, perform a search
3. **Expected:** Panel background, text, borders, result cards all render correctly in light theme — no unreadable text, no invisible borders
4. Switch to Dark theme
5. **Expected:** Panel renders correctly in dark theme with proper contrast

## Edge Cases

### Rapid Search Replacement

1. Open search panel
2. Type "resistor", wait for results
3. Immediately clear input and type "capacitor"
4. **Expected:** Results update to capacitors, no stale resistor results remain mixed in

### Panel State After Page Reload

1. Open search panel
2. Reload the page (F5)
3. **Expected:** Panel is closed after reload (no persistence of panel open state). Toolbar 🔍 button is in default (inactive) state.

### Debug Surface Verification

1. Open browser DevTools console
2. Open search panel and search "LED"
3. Type `window.__jlcpcbSearch` in console
4. **Expected:** Object with `lastQuery: "LED"`, `resultCount: <number>`, `lastError: null`, `visible: true`
5. Close the panel
6. Type `window.__jlcpcbSearch.visible` in console
7. **Expected:** `false`

### OBJ Model Count After 3D Toggle

1. Switch to 3D view
2. Open console, type `window.__renderer3d.objModelCount`
3. **Expected:** `0` (no OBJ models loaded yet)
4. Switch back to 2D, then back to 3D
5. **Expected:** `objModelCount` resets to `0` on each 3D init (no stale state)

## Failure Signals

- Search panel doesn't appear when clicking 🔍 button — check `jlcpcb-panel.ts` import in `main.ts`
- No results for valid queries — check network tab for `jlcsearch.tscircuit.com` requests, verify API is up
- Console errors on panel open/close — DOM element IDs may have changed in `index.html`
- `__jlcpcbSearch` undefined — `initSearchPanel()` not called in `main.ts` init sequence
- Panel overlaps other UI elements — z-index ordering issue (should be 100, below PM at 150 and prefs at 200)
- 3D model fetch crashes — check `loadComponentFromOBJ()` null handling in `renderer3d.ts`

## Requirements Proved By This UAT

- LIB-05 — JLCPCB API integration: search returns real component data with part metadata
- LIB-01 — Component search by name/value: unified search input queries jlcsearch API
- LIB-09 — Component metadata: results show manufacturer, package, datasheet links, price, stock
- LIB-03 — 3D model association: EasyEDA OBJ pipeline can load geometry for LCSC parts (CORS-dependent)

## Not Proven By This UAT

- Full 3D model rendering end-to-end from localhost (EasyEDA CORS blocks it — works only in production)
- Automatic 3D model association based on board component LCSC part numbers (requires component→LCSC mapping in the DSL/snapshot)
- BOM cost estimation from search results (data is available but no BOM aggregation UI)
- Library import from search results (search is browse-only, no "add to library" flow)

## Notes for Tester

- **CORS limitation**: When testing locally, EasyEDA 3D model fetch will fail with a CORS error. This is expected — the EasyEDA API doesn't allow requests from `localhost`. The error path handles this gracefully. To test full 3D model loading, deploy to a production domain or use a CORS proxy.
- **jlcsearch API**: The tscircuit jlcsearch API is free and auth-less. If it's down, all search tests will show "No results" — check `https://jlcsearch.tscircuit.com` directly.
- **Price format**: Prices show 4 decimal places (e.g., $0.0017) because JLCPCB component prices are often fractions of a cent.
- **Debounce timing**: The 300ms debounce means you need to wait ~500ms after typing before results appear. Don't assume the search is broken if results aren't instant.
