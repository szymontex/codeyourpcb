# S03 ("E2E Regression Tests") — Research

**Date:** 2026-03-19

## Summary

S03 is straightforward — it adds targeted Playwright E2E tests to the existing test suite and wires them into the quality-gate script so CI catches two classes of regression: (1) UI freeze during routing (main thread blocked), and (2) routing quality degradation (unrouted connections > 0 on Blink LED). Both S01 and S02 are complete, providing all prerequisites: the Web Worker routing infrastructure with `window.__routingWorker` debug surface (S01) and the PathFinder ghost-cell fix with rebuilt WASM binary (S02).

The existing `autoroute-worker.spec.ts` already has 3 tests from S01 proving overlay visibility, cancel, and basic result validity. S03's job is to extend this with tests that specifically assert the regression contracts — UI responsiveness (R205) and 0 unrouted quality (R206) — using the Blink LED template (the board proven in S02's cargo tests). The existing patterns (fixture loading via `__loadBoard`, debug surface queries, WASM availability checks) are well-established and should be reused directly.

## Recommendation

Create a new test file `viewer/e2e/autoroute-regression.spec.ts` (not extending the existing `autoroute-worker.spec.ts`) that contains the CI regression gate tests. Keep these separate because they have a different purpose: S01's tests are smoke tests for the worker mechanism, while S03's tests are regression gates that should fail CI when quality degrades. Use the Blink LED template (`viewer/public/templates/blink.cypcb`) as the routing fixture since it's the board with the 0-unrouted guarantee from S02. Tests that require real WASM should skip gracefully with a clear message in non-WASM environments (same pattern as S01's test 3).

## Implementation Landscape

### Key Files

- `viewer/e2e/autoroute-worker.spec.ts` — Existing S01 smoke tests (3 tests). Pattern reference for WASM checks, fixture loading, debug surface queries. Do NOT modify.
- `viewer/e2e/autoroute-regression.spec.ts` — **New file.** The CI regression gate tests for R205 and R206.
- `viewer/public/templates/blink.cypcb` — Blink LED template (8 components, 7 nets, 25 connections). Identical to `examples/blink.cypcb` used in `test_blink_led_zero_unrouted`.
- `viewer/src/main.ts` — Hosts `window.__routingWorker` debug surface (`active` getter, `lastResult` string). Also `window.__loadBoard(source)` for loading boards in tests. Status text at `#status-text` shows routing result info.
- `viewer/src/routing-worker.ts` — Worker posts `{type:'route-result', snapshot, routeResult}`. `routeResult` is JSON: `{"ok":true,"routed":N,"unrouted":N}` on success.
- `viewer/playwright.config.ts` — Config: `testDir: './e2e'`, `baseURL: http://localhost:4321`, headless Chromium, Vite dev server on port 4321.
- `scripts/quality-gate.sh` — Runs `npx playwright test` as stage 6/8. New test file will be auto-discovered (Playwright scans `./e2e/**/*.spec.ts`).

### Build Order

1. **Create the Blink LED fixture file** — Copy `viewer/public/templates/blink.cypcb` to `viewer/e2e/fixtures/blink.cypcb` so tests can load it via `fs.readFileSync` without depending on the dev server's `/templates/` route. (Follows pattern from `autoroute-worker.spec.ts` which reads `fixtures/routing-test.cypcb`.)

2. **Write `autoroute-regression.spec.ts`** — Three tests:
   - **"UI responsive during routing — overlay visible and interactive"** (R205): Load Blink LED via `__loadBoard`, click `#route-btn`, immediately assert `#routing-status` is visible AND `__routingWorker.active === true`. This proves the main thread is free (if WASM ran synchronously, these assertions could never execute during routing). Wait for completion, assert overlay hides.
   - **"routing result has 0 unrouted on Blink LED"** (R206): Load Blink LED, route via worker, wait for `__routingWorker.lastResult` to be non-null, parse JSON, assert `parsed.ok === true` and `parsed.unrouted === 0`. Also assert `parsed.routed > 0` as a sanity check.
   - **"status text reflects routing completion without errors"** (R206 secondary): After routing completes, check `#status-text` content shows "Routed N segments" without "unrouted" substring and without "failed".

   All three tests skip gracefully in non-WASM environments using the established `isWasmAvailable()` pattern (check `#status-text` for "WASM").

3. **Verify quality-gate picks up new tests** — `scripts/quality-gate.sh` stage 6 runs `npx playwright test` which auto-discovers all `e2e/*.spec.ts` files. No script modification needed.

### Verification Approach

- `cd viewer && npx playwright test e2e/autoroute-regression.spec.ts` — Run the new tests in isolation. In WASM-enabled environment: all 3 pass. In mock environment: all 3 skip.
- `cd viewer && npx playwright test` — Run full E2E suite to verify no regressions from new file.
- `npx tsc --noEmit` — TypeScript check (no new types needed, but verify imports compile).
- Verify tests fail correctly by temporarily breaking them: e.g., change `parsed.unrouted === 0` to `parsed.unrouted === 999` and confirm test fails — this proves the assertion is wired correctly.

## Constraints

- **WASM availability**: These tests require real WASM (`cypcb_render_bg.wasm`). In environments where WASM is not available (403, mock engine), tests must skip gracefully — never fail CI for infrastructure reasons.
- **Routing time**: Blink LED routes in <2s via Worker (S02 measured 45 segments, 6 vias). Test timeouts should be generous (60-120s) but the actual wait should be short.
- **Serial execution**: Playwright config has `fullyParallel: false` — tests run serially. New test file won't cause parallel conflicts with existing tests.

## Common Pitfalls

- **Checking `__routingWorker.lastResult` too early** — Must wait for worker completion, not just for `lastResult` to be non-null. Use `page.waitForFunction(() => (window as any).__routingWorker?.lastResult !== null, { timeout: 120_000 })` pattern from S01's test 3.
- **WASM skip condition** — The pattern checks `#status-text` for "WASM" keyword. If status text wording changes (e.g., "Ready (WebAssembly)"), the skip would break. Use the established `isWasmAvailable()` helper from S01.
- **Stale status text** — After routing completes, `statusText` updates to the routing result message but then resets to "Ready" after 5 seconds (setTimeout in main.ts). Assert status text immediately after `__routingWorker.active` becomes false, not after a delay.
