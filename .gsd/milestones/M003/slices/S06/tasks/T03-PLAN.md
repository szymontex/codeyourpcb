---
estimated_steps: 4
estimated_files: 2
---

# T03: E2E tests with Playwright route interception

**Slice:** S06 — JLCPCB Integration & 3D Models
**Milestone:** M003

## Description

Create Playwright E2E tests that exercise the full JLCPCB search and 3D model loading flow using route interception to mock external API responses. Tests must be deterministic and CI-safe — no real API calls.

Uses Playwright's `page.route()` to intercept requests to `jlcsearch.tscircuit.com`, `easyeda.com`, and `modules.easyeda.com`, returning canned responses that match the real API shapes documented in S06-RESEARCH.md.

## Steps

1. Create `viewer/e2e/jlcpcb-search.spec.ts` with route interception setup in `beforeEach`:
   - `**/jlcsearch.tscircuit.com/api/search*` → return mock search results JSON (3 components with realistic metadata)
   - `**/easyeda.com/api/products/*/components*` → return mock component data with 3D UUID in shape array
   - `**/modules.easyeda.com/3dmodel/*` → return mock EasyEDA OBJ text (simple cube with 2 materials)
   - Load a minimal board via `__loadBoard()` to dismiss project manager

2. Write search flow tests:
   - "search panel opens and closes via toolbar button" — click 🔍, panel visible; click again, panel hidden
   - "search returns results with metadata" — type "0805 10k" in search input, wait for results, verify result count and content (LCSC#, manufacturer, package displayed)
   - "empty search shows no-results message" — intercept with empty response, type query, verify "No results" message
   - "API error shows error state" — intercept with 500 status, type query, verify error message displayed
   - "search is debounced" — type rapidly, verify only one API request made (count intercepted requests)

3. Write 3D model loading test:
   - "component click loads 3D model when 3D view active" — activate 3D view, open search panel, search, click first result, verify via `__renderer3d.objModelCount >= 1` or `__jlcpcbSearch` debug surface

4. Run full test suite: `npx playwright test e2e/jlcpcb-search.spec.ts` then `npx playwright test` to verify no regressions.

## Must-Haves

- [ ] All external API calls intercepted — zero real network requests to jlcsearch/easyeda
- [ ] Search results display test verifies actual content (LCSC number, manufacturer visible)
- [ ] Error state test verifies user-visible error message
- [ ] Debounce test verifies single API call for rapid input
- [ ] 3D model load test verifies via debug surface (not pixel comparison)
- [ ] No regressions in existing E2E suite

## Verification

- `npx playwright test e2e/jlcpcb-search.spec.ts` — all tests pass
- `npx playwright test` — full suite passes (zero failures across all spec files)

## Inputs

- `viewer/src/jlcpcb-panel.ts` — search panel DOM structure, `__jlcpcbSearch` debug surface (T02)
- `viewer/src/renderer3d.ts` — `__renderer3d.objModelCount` debug surface (T01)
- `viewer/e2e/*.spec.ts` — existing test patterns, `__loadBoard()` helper, `activate3D()` helper from `three-d-view.spec.ts`
- S06-RESEARCH.md — API response shapes for mock data construction

## Observability Impact

- **No new runtime signals** — this task adds tests only, no production code changes.
- **Test diagnostic surface:** All tests use route interception counters and `window.__jlcpcbSearch` / `window.__renderer3d` debug surfaces built in T01/T02. A failing test will show which intercepted route was/wasn't hit and which debug surface value didn't match expectations.
- **Failure visibility:** Playwright trace + screenshot on failure (configured in `playwright.config.ts`). Test names map directly to the user flow being validated (search open/close, results display, error state, debounce, 3D load).
- **Future agent inspection:** Run `npx playwright test e2e/jlcpcb-search.spec.ts --reporter=list` to see pass/fail per test. Add `--trace on` for full trace on all tests.

## Expected Output

- `viewer/e2e/jlcpcb-search.spec.ts` — NEW: 5-6 E2E tests with route interception (~150-200 lines)
- Full E2E suite passing with no regressions
