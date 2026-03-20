# S03: E2E Regression Tests

**Goal:** CI has Playwright E2E tests that catch UI responsiveness regressions during routing and routing quality degradation (unrouted > 0) on Blink LED.
**Demo:** `cd viewer && npx playwright test e2e/autoroute-regression.spec.ts` passes — all 3 tests either pass (WASM env) or skip gracefully (non-WASM env). Full E2E suite still green. Quality gate auto-discovers the new tests.

## Must-Haves

- New file `viewer/e2e/autoroute-regression.spec.ts` with 3 regression gate tests (separate from S01's smoke tests)
- Blink LED fixture at `viewer/e2e/fixtures/blink.cypcb` for test loading
- Test 1 (R205): Proves UI is responsive during routing — overlay visible AND `__routingWorker.active === true` mid-routing on Blink LED board
- Test 2 (R206): Asserts routing result has `unrouted === 0` and `ok === true` on Blink LED via worker
- Test 3 (R206 secondary): Asserts `#status-text` shows routing completion without "unrouted" or "failed" substrings
- All 3 tests skip gracefully in non-WASM environments (same `isWasmAvailable()` pattern as S01)
- Existing `autoroute-worker.spec.ts` is NOT modified
- Full E2E suite (`npx playwright test`) passes with the new file included

## Proof Level

- This slice proves: operational (E2E regression gates in real browser)
- Real runtime required: yes (Playwright + Vite dev server + WASM for full pass)
- Human/UAT required: no

## Verification

- `cd viewer && npx playwright test e2e/autoroute-regression.spec.ts` — all 3 tests pass (WASM) or skip (non-WASM), zero failures
- `cd viewer && npx playwright test` — full suite passes (no regressions from new file)
- `cd viewer && npx tsc --noEmit` — TypeScript compiles with zero errors
- Quality gate stage 6 (`scripts/quality-gate.sh`) auto-discovers `autoroute-regression.spec.ts` (no script changes needed)

## Observability / Diagnostics

- Runtime signals: `window.__routingWorker.active` (live boolean), `window.__routingWorker.lastResult` (JSON string), `#status-text` content, `#routing-status` overlay visibility
- Inspection surfaces: Playwright trace files in `test-results/` on failure (retain-on-failure config)
- Failure visibility: Test names clearly indicate which regression class failed — "UI responsive" (R205) vs "0 unrouted" (R206) vs "status text" (R206 secondary)
- Redaction constraints: none

## Integration Closure

- Upstream surfaces consumed: `window.__routingWorker` debug surface (S01), `window.__loadBoard()` (S01), rebuilt WASM binary with PathFinder fix (S02), `viewer/public/templates/blink.cypcb` (S02 fixture)
- New wiring introduced in this slice: none — Playwright auto-discovers `e2e/*.spec.ts`
- What remains before the milestone is truly usable end-to-end: S04 (variant generation + tuning via worker)

## Tasks

- [x] **T01: Write Blink LED regression test suite** `est:45m`
  - Why: Core deliverable — creates the fixture and all 3 regression gate tests that satisfy R205 and R206
  - Files: `viewer/e2e/fixtures/blink.cypcb`, `viewer/e2e/autoroute-regression.spec.ts`
  - Do: Copy `viewer/public/templates/blink.cypcb` to `viewer/e2e/fixtures/blink.cypcb`. Write `autoroute-regression.spec.ts` with 3 tests following S01's patterns (loadFixture, isWasmAvailable, beforeEach with PM dismissal). Test 1: load Blink LED, click Route, assert `#routing-status` visible AND `__routingWorker.active === true` during routing, wait for completion, assert overlay hidden. Test 2: load Blink LED, route via worker, wait for `__routingWorker.lastResult` non-null, parse JSON, assert `ok === true`, `unrouted === 0`, `routed > 0`. Test 3: after routing completes, check `#status-text` for positive result — must NOT contain "unrouted" or "failed". All tests skip via `isWasmAvailable()` check in non-WASM envs. Use 120s timeouts for routing wait. Assert status text immediately after `__routingWorker.active` becomes false (before the 5s auto-reset).
  - Verify: `cd viewer && npx playwright test e2e/autoroute-regression.spec.ts` — zero failures (tests pass or skip)
  - Done when: All 3 tests exist, TypeScript compiles, test run shows 0 failures

- [x] **T02: Verify full E2E suite integration and quality gate** `est:15m`
  - Why: Proves the new test file doesn't break existing tests and quality gate auto-discovers it
  - Files: `viewer/e2e/autoroute-regression.spec.ts` (read-only), `scripts/quality-gate.sh` (read-only verification)
  - Do: Run `cd viewer && npx tsc --noEmit` to verify TypeScript. Run `cd viewer && npx playwright test` to verify full suite (existing autoroute-worker tests + new regression tests). Verify `scripts/quality-gate.sh` stage 6 runs `npx playwright test` which scans `./e2e/**/*.spec.ts` — confirm no changes needed. If any test fails, diagnose and fix in the regression test file only (never modify autoroute-worker.spec.ts or production code).
  - Verify: `cd viewer && npx playwright test` exits 0; `cd viewer && npx tsc --noEmit` exits 0
  - Done when: Full E2E suite passes with both test files, TypeScript clean, quality gate confirmed to auto-discover

## Files Likely Touched

- `viewer/e2e/fixtures/blink.cypcb` — new (copied from `viewer/public/templates/blink.cypcb`)
- `viewer/e2e/autoroute-regression.spec.ts` — new (3 regression gate tests)
