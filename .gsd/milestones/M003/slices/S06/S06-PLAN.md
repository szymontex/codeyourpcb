# S06: JLCPCB Integration & 3D Models

**Goal:** User can search JLCPCB/LCSC components from within the app, view part metadata, and components with 3D models render real geometry in the 3D view (replacing placeholder boxes).
**Demo:** Type "0805 10k" in search panel → see resistor results with price/stock/package → click a result → 3D view shows real component model (or graceful "no model" indicator).

## Must-Haves

- Search panel queries jlcsearch API (`jlcsearch.tscircuit.com/api/search`) and displays results with LCSC part number, manufacturer, package, price, stock, datasheet link
- Toolbar has a search button that opens/closes the component search panel
- Custom OBJ parser handles EasyEDA's non-standard format (inline `newmtl`/`endmtl`, `f v// v// v//` faces, computed normals)
- EasyEDA 3D model pipeline: LCSC part number → component API → 3D UUID extraction → OBJ fetch → parse → Three.js geometry
- `loadComponentFromOBJ(objText, refdes)` method on Renderer3D replaces placeholder box with parsed OBJ model
- Search input debounced (300ms minimum)
- Graceful error handling: API down → error message in panel; no 3D model → keep placeholder box + indicator
- 3D models disposed on `clearBoardGroup()` (no memory leaks)
- E2E tests with Playwright route interception covering search flow and 3D model loading

## Proof Level

- This slice proves: integration (browser → external API → 3D render pipeline)
- Real runtime required: yes (API calls, Three.js rendering)
- Human/UAT required: no (E2E with mocked API responses sufficient)

## Verification

- `npx vitest run` — all unit tests pass including new jlcsearch client, EasyEDA client, and OBJ parser tests
- `npx playwright test e2e/jlcpcb-search.spec.ts` — E2E tests pass with mocked API responses: search returns results, component selection triggers 3D model load, error states display correctly
- `npx playwright test` — full E2E suite passes (no regressions)

## Observability / Diagnostics

- Runtime signals: `[JLCPCB] Search: "${query}" → ${count} results` console log on search completion; `[3D] OBJ loaded for ${refdes}` on successful model load; `[3D] OBJ parse failed: ${error}` on parse failure
- Inspection surfaces: `window.__jlcpcbSearch` debug surface with `lastQuery`, `resultCount`, `lastError` for E2E; `window.__renderer3d` extended with `objModelCount`
- Failure visibility: search panel shows inline error message on API failure; 3D falls back to placeholder box on model load failure
- Redaction constraints: none (no secrets involved — all APIs are auth-free)

## Integration Closure

- Upstream surfaces consumed: `renderer3d.ts` `loadComponentModel()` pattern and `clearBoardGroup()` disposal (S02); `settings.ts` `getPreference/setPreference` (S04); `project-manager.ts` overlay pattern (S05); `index.html` toolbar structure (S04)
- New wiring introduced in this slice: `jlcpcb.ts` module imported in `main.ts`, toolbar button handler, renderer3d extended with `loadComponentFromOBJ()`
- What remains before the milestone is truly usable end-to-end: S07 (polish, bugs, full verification gate)

## Tasks

- [x] **T01: JLCPCB search client, EasyEDA 3D pipeline, and OBJ parser** `est:1h`
  - Why: Core logic that everything else depends on — the API clients, OBJ parser, and renderer3d extension. Must be unit-testable in isolation before wiring UI.
  - Files: `viewer/src/jlcpcb.ts`, `viewer/src/easyeda-obj-parser.ts`, `viewer/src/renderer3d.ts`, `viewer/src/__tests__/jlcpcb.test.ts`, `viewer/src/__tests__/easyeda-obj-parser.test.ts`
  - Do: Build jlcsearch API client (search function with debounce-ready design), EasyEDA component API client (LCSC→UUID→OBJ pipeline), custom OBJ parser that extracts vertices/faces/materials from EasyEDA format into Three.js BufferGeometry+MeshStandardMaterial, add `loadComponentFromOBJ(objText, refdes)` to renderer3d.ts alongside existing `loadComponentModel()`. Prepend `C` to bare LCSC numbers for EasyEDA API. Handle `d 0.0` as opaque. Compute face normals from vertex positions.
  - Verify: `npx vitest run` — new unit tests pass for OBJ parsing (vertices, faces, materials, edge cases) and API response parsing
  - Done when: OBJ parser correctly handles EasyEDA sample data, API clients parse real response shapes, renderer3d has working `loadComponentFromOBJ()` method

- [x] **T02: Search panel UI, toolbar button, and 3D model wiring** `est:1h`
  - Why: The user-facing surface — search panel DOM, toolbar integration, event wiring between search results → EasyEDA 3D pipeline → renderer3d. Without this, the logic from T01 has no UI.
  - Files: `viewer/index.html`, `viewer/src/jlcpcb-panel.ts`, `viewer/src/main.ts`
  - Do: Add search panel HTML (overlay at z-index 100, search input + results list + loading/error states), add 🔍 toolbar button, create `jlcpcb-panel.ts` module with DOM construction and event handling (debounced search, result click → fetch 3D model → call renderer3d), wire in main.ts (import + init + button handler + 3D model callback). Results show: LCSC#, manufacturer, package, value/description, price, stock, datasheet link. Component click fetches 3D model only when 3D view is active. Expose `__jlcpcbSearch` debug surface.
  - Verify: Dev server running, search panel opens/closes, search returns results (manual with real API), 3D model loads for component with available model
  - Done when: Full user flow works — toolbar button → search panel → type query → see results → click result → 3D model appears (or graceful fallback)

- [x] **T03: E2E tests with Playwright route interception** `est:45m`
  - Why: External APIs can't be hit reliably in CI. Playwright route interception mocks jlcsearch and EasyEDA responses, proving the full flow works with known data. Locks down the feature against regressions.
  - Files: `viewer/e2e/jlcpcb-search.spec.ts`
  - Do: Create E2E test file with route interception for `jlcsearch.tscircuit.com/api/search*` and `easyeda.com/api/products/*/components*` and `modules.easyeda.com/3dmodel/*`. Test cases: search returns results and displays them, empty search shows no-results message, API error shows error state, component click triggers 3D model fetch (verify via `__renderer3d.objModelCount` or `__jlcpcbSearch` debug surface), search input is debounced (rapid typing triggers single request).
  - Verify: `npx playwright test e2e/jlcpcb-search.spec.ts` passes; `npx playwright test` full suite passes
  - Done when: All E2E tests pass with mocked APIs, no regressions in existing test suite

## Files Likely Touched

- `viewer/src/jlcpcb.ts` — NEW: jlcsearch API client + EasyEDA API client
- `viewer/src/easyeda-obj-parser.ts` — NEW: custom OBJ parser for EasyEDA format
- `viewer/src/jlcpcb-panel.ts` — NEW: search panel DOM + event handling
- `viewer/src/renderer3d.ts` — extend with `loadComponentFromOBJ()`, `objModelCount` diagnostic
- `viewer/src/main.ts` — wire jlcpcb-panel module + toolbar button
- `viewer/index.html` — search panel HTML + toolbar button + CSS
- `viewer/src/__tests__/jlcpcb.test.ts` — NEW: unit tests for API client response parsing
- `viewer/src/__tests__/easyeda-obj-parser.test.ts` — NEW: unit tests for OBJ parser
- `viewer/e2e/jlcpcb-search.spec.ts` — NEW: E2E tests with route interception
