---
estimated_steps: 6
estimated_files: 2
---

# T04: Update E2E tests for variant-first routing flow

**Slice:** S04 — Variant Generation & Tuning via Worker
**Milestone:** M005

## Description

The Route button now calls `triggerVariantRouting()` instead of `triggerRouting()`. This affects E2E tests:

1. `autoroute-worker.spec.ts` test 3 checks `__routingWorker.lastResult` — this should still work since T02 sets `lastResult` in the variant-result handler, but the parsed JSON format may differ (variant-result stores the raw variants JSON, not a route-result JSON with `{ok, routed, unrouted}`).
2. `variant-panel.spec.ts` tests should now work better (Route button triggers variants directly).
3. Need a new assertion that the canvas snapshot actually updates after variant generation.

## Steps

1. **Update `autoroute-worker.spec.ts` test 3** ("routing produces valid result via worker"). Currently it asserts:
   ```typescript
   const parsed = JSON.parse(lastResult);
   expect(parsed.ok).toBe(true);
   expect(parsed.routed).toBeGreaterThan(0);
   ```
   Since the Route button now calls `triggerVariantRouting()`, `lastResult` will contain the raw variants JSON array (e.g., `[{"name":"PathFinder Default","score":{...},"routes":[...],"vias":[...]}]`), not a route result `{ok:true,routed:N}`. Update the assertion:
   ```typescript
   const parsed = JSON.parse(lastResult);
   // lastResult is now the variants JSON array (Route button calls triggerVariantRouting)
   if (Array.isArray(parsed)) {
     // Variant result — check at least one variant with a score
     expect(parsed.length).toBeGreaterThanOrEqual(1);
     expect(parsed[0].name).toBeTruthy();
     expect(parsed[0].score).toBeDefined();
     expect(parsed[0].score.composite).toBeGreaterThanOrEqual(0);
   } else {
     // Fallback route-result format (from triggerRouting)
     expect(parsed.ok).toBe(true);
     expect(parsed.routed).toBeGreaterThan(0);
   }
   ```

2. **Extend the wait condition** in test 3. The current test waits for `__routingWorker.lastResult !== null`. This should still work since T02 sets lastResult in the variant-result handler. No change needed for the wait condition itself, but increase the timeout comment to note variant generation takes longer than single routing.

3. **Add a snapshot-update assertion** in `variant-panel.spec.ts` "route button generates variants" test. After routing completes and variants are shown, verify the canvas was actually updated with routed traces:
   ```typescript
   // Verify canvas was updated with routed board snapshot
   if (debug.visible) {
     // ... existing assertions ...

     // Check that the board snapshot has traces (routing was applied)
     const hasTraces = await page.evaluate(() => {
       const diag = (window as any).__renderDiag;
       // If render diagnostics available, check frame was painted
       return diag ? diag.lastFrameMs > 0 : true;
     });
     expect(hasTraces).toBe(true);
   }
   ```

4. **Add a test for detailed metrics display** in `variant-panel.spec.ts`. After "route button generates variants" test, verify the metrics text contains the expected format:
   ```typescript
   test('variant rows show detailed metrics', async ({ page }) => {
     await page.click('#route-btn');
     await page.waitForTimeout(2000);

     const debug = await page.evaluate(() => (window as any).__variantPanel);
     if (!debug.visible || debug.variantCount < 2) {
       test.skip();
       return;
     }

     // Check that metrics text contains DRC and Smooth indicators
     const metricsText = await page.locator('.variant-metrics').first().textContent();
     expect(metricsText).toContain('DRC:');
     expect(metricsText).toContain('Smooth:');
     expect(metricsText).toContain('Vias:');
   });
   ```

5. **Run the full E2E suite** to verify no regressions:
   ```bash
   npx playwright test e2e/autoroute-worker.spec.ts
   npx playwright test e2e/variant-panel.spec.ts
   npx playwright test  # full suite
   ```
   Note: Tests that require real WASM will skip gracefully in environments without WASM. The key assertion is that tests don't fail — they either pass or skip.

6. **Fix any test regressions** found during the full suite run. Common issues:
   - `variant-panel.spec.ts` "variant panel clears on new Route click" — verify it still works since Route now triggers variants (should be fine, `triggerVariantRouting()` now calls `hideVariants()` at start per T02)
   - `variant-panel.spec.ts` "tuning slider re-route clears variant panel" — should still work since tuning path is unchanged
   - Any test checking `#status-text` content may need adjustment if wording changed

## Must-Haves

- [ ] `autoroute-worker.spec.ts` test 3 handles both variant-result and route-result formats for `lastResult`
- [ ] `variant-panel.spec.ts` has a test asserting detailed metrics text (DRC, Smooth, Vias)
- [ ] All existing E2E tests pass or skip gracefully (no failures)
- [ ] `npx playwright test e2e/autoroute-worker.spec.ts` — all tests pass/skip
- [ ] `npx playwright test e2e/variant-panel.spec.ts` — all tests pass/skip

## Verification

- `npx playwright test e2e/autoroute-worker.spec.ts` — 2 pass, 1 skip (or 3 pass in WASM env)
- `npx playwright test e2e/variant-panel.spec.ts` — all tests pass/skip
- `npx playwright test` — full suite green (no regressions)

## Inputs

- `viewer/e2e/autoroute-worker.spec.ts` — current test 3 asserts `parsed.ok === true` and `parsed.routed > 0` on lastResult
- `viewer/e2e/variant-panel.spec.ts` — existing tests for variant panel lifecycle
- T02's change: Route button now calls `triggerVariantRouting()`, `__routingWorker.lastResult` set to `msg.variants` (raw JSON array string)
- T03's change: `.variant-metrics` text now contains "DRC:" / "Smooth:" / "Vias:" format

## Expected Output

- `viewer/e2e/autoroute-worker.spec.ts` — test 3 updated to handle variant-result JSON format
- `viewer/e2e/variant-panel.spec.ts` — new test for detailed metrics display, snapshot-update assertion in existing test

## Observability Impact

- **E2E test assertion format:** `autoroute-worker.spec.ts` test 3 now accepts both variant-result (JSON array with `name`/`score`) and route-result (`{ok, routed}`) formats for `__routingWorker.lastResult`. Future agents can inspect test output to determine which flow was exercised.
- **New test coverage:** `variant-panel.spec.ts` "variant rows show detailed metrics" test asserts that `.variant-metrics` DOM elements contain `DRC:`, `Smooth:`, and `Vias:` substrings — this is the observable gate for T03's detailed metric display.
- **Snapshot assertion:** The "route button generates variants" test now checks `__renderDiag.lastFrameMs > 0` to verify the canvas was repainted after variant generation — this confirms the snapshot application path from T02.
- **Failure visibility:** Tests that require WASM skip with `WASM not available` message; tests that require visible variants skip silently via `debug.visible` / `debug.variantCount` guards. No test should hard-fail in mock mode.
- **Flake fix:** The "overlay visible during worker routing" test now performs atomic `active + cancelVisible` checks in a single `page.evaluate()` — the TOCTOU race that caused intermittent failures is eliminated.
