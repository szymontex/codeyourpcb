---
estimated_steps: 5
estimated_files: 4
---

# T02: Search panel UI, toolbar button, and 3D model wiring

**Slice:** S06 — JLCPCB Integration & 3D Models
**Milestone:** M003

## Description

Build the user-facing search panel as a side panel overlay, add a toolbar search button, and wire the full flow: user types query → debounced API call → results rendered in panel → user clicks result → EasyEDA 3D model fetched → loaded into renderer3d. Follows the overlay/callback pattern established by `project-manager.ts`.

The search panel is a right-side overlay (not modal — user can still see the board) with a search input, results list, loading spinner, and error/empty states. Each result row shows LCSC number, manufacturer, package, key attributes (e.g., resistance value), price, stock count, and a datasheet link. Clicking a result fetches the 3D model via the EasyEDA pipeline from T01 and calls `loadComponentFromOBJ()` if the 3D view is active.

## Steps

1. Add search panel HTML to `viewer/index.html` — right-side panel (`position: fixed; right: 0; top: 41px; width: 360px; height: calc(100% - 41px); z-index: 100`) with: search input (placeholder "Search JLCPCB parts..."), results container (scrollable), loading indicator, error message area, close button. Style with CSS variables for theme compatibility. Add `🔍` toolbar button (`id="jlcpcb-search-btn"`) after the ⚙ Prefs button.

2. Create `viewer/src/jlcpcb-panel.ts` — module with `initSearchPanel(callbacks)` / `showSearchPanel()` / `hideSearchPanel()` / `toggleSearchPanel()`. DOM wiring: search input `input` event → debounce 300ms → call `searchComponents()` from `jlcpcb.ts` → render results. Result rendering: create DOM elements per result with LCSC#, mfr, package, attributes summary, price (formatted to 4 decimal places), stock count, datasheet `<a>` link. Result click handler calls `onComponentSelect(component)` callback. Loading state: show spinner during search, hide on complete. Error state: show inline error message. Empty state: "No results found" message. Expose `window.__jlcpcbSearch = { lastQuery, resultCount, lastError }` debug surface.

3. Wire in `viewer/src/main.ts` — import `jlcpcb-panel.ts`, call `initSearchPanel()` with callback that: (a) on component select, calls `fetch3DModel(component.lcsc)` from `jlcpcb.ts`, (b) if OBJ text returned and 3D view is active, calls `renderer3d.loadComponentFromOBJ(objText, refdes)` where refdes comes from the selected component context. Add click handler for `#jlcpcb-search-btn` → `toggleSearchPanel()`. Close panel on Escape if open.

4. Add panel open/close keyboard shortcut (Ctrl+J or similar) and ensure panel closes when project manager opens and vice versa (avoid overlay stacking). Panel state (open/closed) does not persist to settings — always starts closed.

5. Test manually with dev server — open search panel, search "0805 10k", verify results appear with metadata, verify 3D model loads when clicking a result in 3D mode.

## Must-Haves

- [ ] Search panel opens/closes via toolbar button click
- [ ] Search input debounced at 300ms — rapid typing triggers only one API call
- [ ] Results display LCSC number, manufacturer, package, price, stock, and datasheet link
- [ ] Loading state visible during API fetch
- [ ] Error state displays inline message (not crash) on API failure
- [ ] Empty results show "No results found" message
- [ ] Component click triggers 3D model fetch + load pipeline (when 3D view active)
- [ ] Panel styled with CSS variables for theme compatibility (dark/light)
- [ ] `window.__jlcpcbSearch` debug surface exposed with `lastQuery`, `resultCount`, `lastError`
- [ ] No overlay stacking conflicts with project manager or preferences modal

## Verification

- Dev server: toolbar shows 🔍 button, clicking opens/closes search panel
- Search "0805 10k" returns results with visible metadata
- Clicking a result in 3D mode triggers console log `[3D] OBJ loaded for ...` (or `[JLCPCB] 3D fetch error` / `No 3D model` for parts without models)
- `window.__jlcpcbSearch.resultCount` > 0 after successful search
- Panel closes on Escape and on toolbar button re-click
- Theme toggle doesn't break panel styling

## Inputs

- `viewer/src/jlcpcb.ts` — `searchComponents()`, `fetch3DModel()`, `JLCPCBComponent` type (T01)
- `viewer/src/easyeda-obj-parser.ts` — imported by jlcpcb.ts (T01)
- `viewer/src/renderer3d.ts` — `loadComponentFromOBJ()` method (T01)
- `viewer/src/project-manager.ts` — overlay pattern reference (show/hide, callback interface)
- `viewer/index.html` — toolbar structure, CSS variable patterns
- `viewer/src/main.ts` — module wiring pattern (import + init + handler)

## Observability Impact

- **New debug surface:** `window.__jlcpcbSearch` with `lastQuery` (string), `resultCount` (number), `lastError` (string|null) — updated after each search or error. E2E tests can read these to verify search flow without DOM scraping.
- **Console logs:** `[JLCPCB] Search: "${query}" → ${count} results` on search completion; `[JLCPCB] 3D fetch error` or `[3D] OBJ loaded for ${refdes}` on component selection. Grep `[JLCPCB]` or `[3D] OBJ` to trace the full search→3D pipeline.
- **Failure visibility:** Search panel shows inline error/empty messages — no silent failures. API errors surface as user-visible text in the panel and update `__jlcpcbSearch.lastError`.
- **Panel state:** Inspect panel open/closed via `document.getElementById('jlcpcb-search-panel').classList.contains('hidden')`.

## Expected Output

- `viewer/src/jlcpcb-panel.ts` — NEW: search panel DOM + event handling (~200-250 lines)
- `viewer/index.html` — toolbar button added, search panel HTML + CSS added
- `viewer/src/main.ts` — jlcpcb-panel import, init call, toolbar button handler, 3D model callback wiring
