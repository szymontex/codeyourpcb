---
estimated_steps: 4
estimated_files: 3
---

# T02: E2E tests for renderer quality and visual verification

**Slice:** S01 — Professional 2D Board Renderer
**Milestone:** M003

## Description

Write E2E tests that objectively verify the professional renderer is working. Uses the `window.__renderDiag` diagnostic surface from T01 to assert on renderer state without fragile pixel comparison. Also adds a dedicated unit test file for `buildPadNetMap()` edge cases (T01's unit tests cover the basics, this covers edge cases and regression scenarios).

## Steps

1. **Write `renderer-quality.spec.ts`** — Playwright E2E test that loads app, waits for Ready status, then verifies:
   - Canvas element exists and has non-zero dimensions
   - `__renderDiag.padNetMapSize > 0` (pad-to-net lookup was built from snapshot)
   - Zoom to close level (simulate wheel events), verify `__renderDiag.lodTier` is 'close' or 'detail'
   - `__renderDiag.textElementsDrawn > 0` at close zoom (pad numbers / net labels being drawn)
   - Zoom to far level, verify `__renderDiag.lodTier` is 'far' and `textElementsDrawn === 0`
   - Trigger net highlight (click a trace), verify `__renderDiag.highlightedNet` is set

2. **Write `pad-net-map.test.ts`** — Vitest unit tests covering edge cases:
   - Empty nets array → empty map
   - Net with no connections → skip
   - Multi-pin component (e.g., IC with 20 pins) → all pins mapped
   - Through-hole component with alphanumeric pins ("A1", "B2")
   - Duplicate pin refs across nets (shouldn't happen but guard against it — last wins)
   - Power net connections (VCC, GND) map correctly

3. **Performance sanity check in E2E** — Add test that loads blink.cypcb, renders at close zoom, and checks `__renderDiag.lastFrameMs < 32` (allowing 2× headroom over 16ms budget for headless rendering overhead). Not a hard performance test, just a sanity gate.

4. **Run full test suite** — Ensure all existing tests still pass alongside new ones. Run vitest and playwright in sequence. Fix any regressions introduced by T01 renderer changes.

## Must-Haves

- [ ] `viewer/e2e/renderer-quality.spec.ts` tests pass in headless Chromium
- [ ] `viewer/src/__tests__/pad-net-map.test.ts` covers empty, single, multi-pin, and edge cases
- [ ] LOD tier transitions verified by E2E (zoom triggers tier change)
- [ ] Existing E2E tests (`board-interaction`, `theme`, `app-load`) still pass
- [ ] No new test flakiness introduced

## Verification

- `cd viewer && npx vitest run --reporter=verbose` — all unit tests pass including new pad-net-map tests
- `cd viewer && npx playwright test e2e/renderer-quality.spec.ts` — new E2E passes
- `cd viewer && npx playwright test` — full E2E suite passes (no regressions)

## Inputs

- `viewer/src/render-config.ts` — `buildPadNetMap()` function to test, `LodTier` type for assertions
- `viewer/src/renderer.ts` — exposes diagnostic surface via `window.__renderDiag`
- `viewer/src/main.ts` — wires diagnostic surface, builds padNetMap on snapshot change
- `viewer/e2e/board-interaction.spec.ts` — existing E2E patterns to follow (page.goto, status wait, evaluate)
- `viewer/src/__tests__/render-config.test.ts` — T01's unit tests to complement (not duplicate)

## Expected Output

- `viewer/e2e/renderer-quality.spec.ts` — new E2E test file with 4–6 test cases covering LOD, pad-net-map presence, text rendering, net highlighting, performance sanity
- `viewer/src/__tests__/pad-net-map.test.ts` — new unit test file with 5–7 test cases for buildPadNetMap edge cases
- All existing tests still green
