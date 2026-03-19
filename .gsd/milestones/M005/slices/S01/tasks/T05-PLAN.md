---
estimated_steps: 5
estimated_files: 1
---

# T05: E2E smoke test — overlay visible and cancel works during Worker routing

**Slice:** S01 — Web Worker WASM Routing
**Milestone:** M005

## Description

Create a Playwright E2E test that proves the main thread is responsive during WASM routing. This is the acceptance test for R201 (main thread never blocked), R202 (spinner visible), and R203 (cancel works). It also catches future regressions — if someone moves WASM back to main thread, the overlay won't be visible during routing and the test fails.

The test loads a board via `__loadBoard()` (established E2E pattern from existing tests), clicks Route, and asserts that the overlay/spinner is visible and the cancel button is clickable DURING routing — not just before or after. This only works if routing is in a Web Worker.

## Steps

1. **Create `viewer/e2e/autoroute-worker.spec.ts`** with Playwright test structure matching existing E2E test conventions (import from `@playwright/test`, load fixtures via `__loadBoard()`).

2. **Test: "overlay visible during worker routing"**
   ```typescript
   test('overlay visible during worker routing', async ({ page }) => {
     // Navigate to app
     await page.goto('http://localhost:4321');
     await page.waitForTimeout(1000);
     
     // Load board via __loadBoard
     const content = fs.readFileSync(FIXTURE_PATH, 'utf-8');
     await page.evaluate((src) => (window as any).__loadBoard(src), content);
     await page.waitForTimeout(500);
     
     // Click Route button
     await page.click('#route-btn');
     
     // Immediately check — overlay should be visible (main thread is free)
     await expect(page.locator('#routing-status')).toBeVisible({ timeout: 2000 });
     await expect(page.locator('#cancel-route-btn')).toBeVisible({ timeout: 2000 });
     
     // Check debug surface — worker should be active
     const active = await page.evaluate(() => (window as any).__routingWorker?.active);
     expect(active).toBe(true);
     
     // Wait for routing to complete (status text changes from "Routing...")
     await page.waitForFunction(
       () => !(window as any).__routingWorker?.active,
       { timeout: 120_000 }
     );
     
     // After completion, overlay should be hidden
     await expect(page.locator('#routing-status')).toBeHidden({ timeout: 5000 });
   });
   ```

3. **Test: "cancel terminates routing immediately"**
   ```typescript
   test('cancel terminates routing immediately', async ({ page }) => {
     await page.goto('http://localhost:4321');
     await page.waitForTimeout(1000);
     
     const content = fs.readFileSync(FIXTURE_PATH, 'utf-8');
     await page.evaluate((src) => (window as any).__loadBoard(src), content);
     await page.waitForTimeout(500);
     
     // Start routing
     await page.click('#route-btn');
     await expect(page.locator('#routing-status')).toBeVisible({ timeout: 2000 });
     
     // Click cancel
     await page.click('#cancel-route-btn');
     
     // Overlay should disappear quickly
     await expect(page.locator('#routing-status')).toBeHidden({ timeout: 3000 });
     
     // Worker should be inactive
     const active = await page.evaluate(() => (window as any).__routingWorker?.active);
     expect(active).toBe(false);
   });
   ```

4. **Test: "routing produces valid result"**
   ```typescript
   test('routing produces valid result via worker', async ({ page }) => {
     await page.goto('http://localhost:4321');
     await page.waitForTimeout(1000);
     
     const content = fs.readFileSync(FIXTURE_PATH, 'utf-8');
     await page.evaluate((src) => (window as any).__loadBoard(src), content);
     await page.waitForTimeout(500);
     
     // Route and wait for completion
     await page.click('#route-btn');
     await page.waitForFunction(
       () => (window as any).__routingWorker?.lastResult !== null,
       { timeout: 120_000 }
     );
     
     // Check the result
     const lastResult = await page.evaluate(() => (window as any).__routingWorker?.lastResult);
     expect(lastResult).toBeTruthy();
     const parsed = JSON.parse(lastResult);
     expect(parsed.ok).toBe(true);
     expect(parsed.routed).toBeGreaterThan(0);
   });
   ```

5. **Use `routing-test.cypcb` fixture** from `viewer/e2e/fixtures/routing-test.cypcb` (same fixture used by existing routing E2E tests). Import path: `path.resolve(__dirname, 'fixtures/routing-test.cypcb')`.

## Must-Haves

- [ ] E2E test file `autoroute-worker.spec.ts` exists with 3 tests
- [ ] "overlay visible" test proves spinner is visible DURING routing (not just before)
- [ ] "cancel works" test proves cancel button terminates routing and hides overlay
- [ ] "routing produces result" test proves worker delivers valid routing result with `routed > 0`
- [ ] All tests use `__loadBoard()` pattern for board loading (consistent with existing E2E tests)
- [ ] Tests use `__routingWorker` debug surface for worker state assertions

## Verification

- `cd viewer && npx playwright test e2e/autoroute-worker.spec.ts --reporter=list` — all 3 tests pass
- Tests genuinely prove main-thread responsiveness (overlay visible + cancel clickable during routing)

## Inputs

- `viewer/e2e/fixtures/routing-test.cypcb` — test fixture board
- `viewer/e2e/routing-ux.spec.ts` — reference for existing E2E test patterns and helper conventions
- `window.__routingWorker` debug surface (from T03): `{ active: boolean, lastResult: string | null }`
- `window.__loadBoard(source)` — established board loading function for E2E tests
- Vite dev server running on port 4321

## Observability Impact

- **New test signals:** Playwright console captures `[Routing] Worker spawned`, `[Routing] Worker WASM ready`, `[Routing] Worker result received` during successful test runs — these act as regression detectors if worker flow breaks
- **Debug surface assertions:** Tests validate `window.__routingWorker.active` and `window.__routingWorker.lastResult` — if these debug surfaces are removed or renamed, tests fail immediately
- **Failure visibility:** Test failures produce Playwright traces (`retain-on-failure`) showing exact DOM state when overlay/cancel assertions fail — inspect `#routing-status` visibility and `#cancel-route-btn` presence

## Expected Output

- `viewer/e2e/autoroute-worker.spec.ts` — new E2E test file with 3 tests proving worker routing works: overlay visibility, cancel functionality, and routing result delivery
