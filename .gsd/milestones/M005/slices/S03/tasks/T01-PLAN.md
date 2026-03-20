---
estimated_steps: 6
estimated_files: 2
---

# T01: Write Blink LED regression test suite

**Slice:** S03 — E2E Regression Tests
**Milestone:** M005

## Description

Create the Blink LED fixture and write `autoroute-regression.spec.ts` with 3 Playwright E2E tests that serve as CI regression gates for R205 (UI responsiveness during routing) and R206 (routing quality — 0 unrouted). These are separate from S01's `autoroute-worker.spec.ts` smoke tests — they validate regression contracts, not mechanism.

**Relevant skills:** `test` (test generation patterns).

## Steps

1. **Copy Blink LED fixture.** Copy `viewer/public/templates/blink.cypcb` to `viewer/e2e/fixtures/blink.cypcb`. This is the board proven by S02's `test_blink_led_zero_unrouted` cargo test to route all 25 connections with 0 unrouted.

2. **Create `viewer/e2e/autoroute-regression.spec.ts`.** Structure:
   - Imports: `{ test, expect }` from `@playwright/test`, `fs`, `path`, `fileURLToPath` from `url`
   - Constant: `BLINK_FIXTURE_PATH` pointing to `fixtures/blink.cypcb`
   - Helper `loadBlinkBoard(page)`: reads fixture via `fs.readFileSync`, loads via `page.evaluate((src) => (window as any).__loadBoard(src), content)`, then `page.waitForTimeout(600)` to let render settle
   - Helper `isWasmAvailable(page)`: checks `#status-text` for "WASM" keyword — returns boolean. Same pattern as `autoroute-worker.spec.ts`.
   - `test.describe('Autoroute Regression Gates', () => { ... })` wrapping all 3 tests

3. **Write test 1: "UI responsive during routing — overlay visible and interactive" (R205).**
   - `test.setTimeout(120_000)`
   - Check `isWasmAvailable()` — if not, `test.skip(true, 'WASM not available — regression tests require real WASM engine')`
   - Call `loadBlinkBoard(page)`
   - Click `#route-btn`
   - **Immediately** assert `#routing-status` is visible (timeout 2000ms). This proves main thread is free — if WASM ran synchronously, these assertions couldn't execute during routing.
   - Check `__routingWorker.active === true` via `page.evaluate`
   - Wait for routing completion: `page.waitForFunction(() => !(window as any).__routingWorker?.active, { timeout: 120_000 })`
   - Assert `#routing-status` is hidden (timeout 5000ms)

4. **Write test 2: "routing result has 0 unrouted on Blink LED" (R206).**
   - `test.setTimeout(120_000)`
   - Check `isWasmAvailable()` — skip if not
   - Call `loadBlinkBoard(page)`
   - Click `#route-btn`
   - Wait for result: `page.waitForFunction(() => (window as any).__routingWorker?.lastResult !== null, { timeout: 120_000 })`
   - Get `lastResult` string via `page.evaluate`
   - `JSON.parse(lastResult)` → assert `parsed.ok === true`, `parsed.unrouted === 0`, `parsed.routed > 0`

5. **Write test 3: "status text reflects routing completion without errors" (R206 secondary).**
   - `test.setTimeout(120_000)`
   - Check `isWasmAvailable()` — skip if not
   - Call `loadBlinkBoard(page)`
   - Click `#route-btn`
   - Wait for worker to finish: `page.waitForFunction(() => !(window as any).__routingWorker?.active, { timeout: 120_000 })`
   - **Immediately** read `#status-text` textContent (before the 5-second auto-reset timer clears it)
   - Assert status text does NOT contain "unrouted" (case-insensitive) — e.g. `expect(statusText?.toLowerCase()).not.toContain('unrouted')`
   - Assert status text does NOT contain "failed" (case-insensitive)
   - Assert status text contains "Routed" (sanity: proves routing ran, not some other state)

6. **Add `beforeEach` hook.** Inside the `describe` block:
   ```typescript
   test.beforeEach(async ({ page }) => {
     await page.goto('/');
     await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
   });
   ```
   This matches S01's pattern and ensures the app is loaded before each test.

## Must-Haves

- [ ] `viewer/e2e/fixtures/blink.cypcb` exists (copied from `viewer/public/templates/blink.cypcb`)
- [ ] `viewer/e2e/autoroute-regression.spec.ts` exists with 3 tests in a single `describe` block
- [ ] Test 1 asserts `#routing-status` visible AND `__routingWorker.active === true` during routing
- [ ] Test 2 asserts `parsed.ok === true` AND `parsed.unrouted === 0` AND `parsed.routed > 0` from `__routingWorker.lastResult`
- [ ] Test 3 asserts `#status-text` contains "Routed" and does NOT contain "unrouted" or "failed"
- [ ] All 3 tests skip gracefully via `isWasmAvailable()` in non-WASM environments
- [ ] `autoroute-worker.spec.ts` is NOT modified

## Verification

- `cd viewer && npx playwright test e2e/autoroute-regression.spec.ts` — exits 0 with 0 failures (3 pass in WASM env, 3 skip in non-WASM env)
- `cd viewer && npx tsc --noEmit` — exits 0

## Inputs

- `viewer/e2e/autoroute-worker.spec.ts` — pattern reference for fixture loading, `isWasmAvailable()`, `__routingWorker` queries, `beforeEach`, and WASM skip
- `viewer/public/templates/blink.cypcb` — the Blink LED template fixture (8 components, 7 nets, 25 connections)
- S01 summary — `window.__routingWorker` has `.active` (live boolean getter) and `.lastResult` (JSON string or null)
- S02 summary — Blink LED produces `{ok:true, routed:N, unrouted:0}` with 45 segments, 6 vias. Status text updates immediately after `active` becomes false, then resets to "Ready" after 5 seconds.

## Observability Impact

- **New signals:** Three named Playwright test results ("UI responsive during routing", "routing result has 0 unrouted", "status text reflects routing completion") visible in CI logs and `test-results/` trace artifacts on failure.
- **How to inspect:** Run `cd viewer && npx playwright test e2e/autoroute-regression.spec.ts --reporter=list` — each test name maps to a specific requirement (R205 or R206). On failure, Playwright retains trace zips in `test-results/` per `playwright.config.ts` retain-on-failure setting.
- **Failure visibility:** Test 1 failure = main thread blocked during routing (R205 regression). Test 2 failure = routing quality degraded, unrouted > 0 (R206 regression). Test 3 failure = status text doesn't reflect clean routing result (R206 secondary). All tests skip with clear reason in non-WASM envs.
- **Runtime surfaces consumed:** `window.__routingWorker.active`, `window.__routingWorker.lastResult`, `#status-text`, `#routing-status` overlay.

## Expected Output

- `viewer/e2e/fixtures/blink.cypcb` — Blink LED fixture for regression tests
- `viewer/e2e/autoroute-regression.spec.ts` — 3 Playwright regression gate tests covering R205 and R206
