# S03: E2E Regression Tests — UAT

**Milestone:** M005
**Written:** 2026-03-19

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: The deliverable is Playwright test code — correctness is proven by running the tests and verifying pass/skip/fail outcomes. No live UI interaction needed beyond what the automated tests themselves perform.

## Preconditions

1. Working directory is the M005 worktree or main branch with S03 changes merged
2. `cd viewer && npm install` has been run (Playwright + dependencies installed)
3. Vite dev server can start (port 5173 available)
4. For full pass (not skip): WASM binary must be accessible via Vite's serve allow list

## Smoke Test

Run `cd viewer && npx playwright test e2e/autoroute-regression.spec.ts --reporter=list` — expect 3 tests listed, all either pass or skip (zero failures).

## Test Cases

### 1. Regression test file exists and compiles

1. Run `cd viewer && npx tsc --noEmit`
2. **Expected:** Exit code 0, no TypeScript errors. `autoroute-regression.spec.ts` compiles alongside all other source files.

### 2. Blink LED fixture matches source template

1. Run `diff viewer/public/templates/blink.cypcb viewer/e2e/fixtures/blink.cypcb`
2. **Expected:** No output (files are identical). Fixture is a clean copy of the source template.

### 3. Regression tests run without failures (non-WASM)

1. Run `cd viewer && npx playwright test e2e/autoroute-regression.spec.ts --reporter=list`
2. **Expected:** 3 tests listed, all 3 skipped with message "WASM not available — regression tests require real WASM engine". Zero failures.

### 4. Full E2E suite passes with new tests included

1. Run `cd viewer && npx playwright test --reporter=list`
2. **Expected:** 109+ passed, 8 skipped, 0 failed. Both `autoroute-worker.spec.ts` and `autoroute-regression.spec.ts` appear in the test list.

### 5. Existing autoroute-worker tests untouched

1. Run `git diff --stat viewer/e2e/autoroute-worker.spec.ts`
2. **Expected:** No output (file unchanged).

### 6. Quality gate auto-discovers regression tests

1. Inspect `scripts/quality-gate.sh` stage 6 — verify it runs `cd viewer && npx playwright test`
2. Inspect `viewer/playwright.config.ts` — verify `testDir: './e2e'` scans all `*.spec.ts` files
3. **Expected:** No changes needed to quality-gate.sh or playwright.config.ts. The new test file is auto-discovered.

### 7. Test names contain requirement IDs (WASM environment)

1. In a WASM-enabled environment, run `cd viewer && npx playwright test e2e/autoroute-regression.spec.ts --reporter=list`
2. **Expected:** Test names include "(R205)" and "(R206)" identifiers. All 3 tests pass (not skip).

### 8. R205 test verifies UI responsiveness during routing (WASM environment)

1. In a WASM-enabled environment, run test 1: "UI responsive during routing — overlay visible and interactive (R205)"
2. **Expected:** Test loads Blink LED, clicks Route, asserts `#routing-status` overlay is visible AND `__routingWorker.active === true` during routing, then asserts overlay is hidden after completion. Passes.

### 9. R206 test verifies 0 unrouted (WASM environment)

1. In a WASM-enabled environment, run test 2: "routing result has 0 unrouted on Blink LED (R206)"
2. **Expected:** Test parses `__routingWorker.lastResult` JSON. Asserts `ok === true`, `unrouted === 0`, `routed > 0`. Passes.

### 10. R206 secondary test verifies clean status text (WASM environment)

1. In a WASM-enabled environment, run test 3: "status text reflects routing completion without errors (R206 secondary)"
2. **Expected:** Status text contains "Routed", does NOT contain "unrouted" or "failed". Passes.

## Edge Cases

### Tests skip cleanly when WASM unavailable

1. Remove or block access to the WASM binary
2. Run `cd viewer && npx playwright test e2e/autoroute-regression.spec.ts`
3. **Expected:** All 3 tests skip with clear skip reason. No failures, no crashes, no hanging.

### New regression tests don't interfere with existing autoroute tests

1. Run `cd viewer && npx playwright test e2e/autoroute-worker.spec.ts --reporter=list`
2. **Expected:** Same results as before S03 — 2 passed, 1 skipped (or 3 passed in WASM env). New file has no side effects.

### Intermittent flake in autoroute-worker is pre-existing

1. If `autoroute-worker.spec.ts` "overlay visible during worker routing" fails intermittently in full suite
2. Re-run in isolation: `cd viewer && npx playwright test e2e/autoroute-worker.spec.ts`
3. **Expected:** Passes in isolation. The flake is a pre-existing timing issue under parallel load, not caused by S03.

## Failure Signals

- TypeScript compilation errors in `autoroute-regression.spec.ts` — broken imports or type mismatches
- Any of the 3 regression tests reporting as "failed" (not "skipped") — test logic error
- Full suite test count drops below 109 — something broke from adding the new file
- `autoroute-worker.spec.ts` shows modifications — S03 should be additive only
- Quality gate stage 6 fails — test file not being auto-discovered

## Requirements Proved By This UAT

- R205 — Test exists that asserts UI overlay visible + worker active during routing (proves main thread not blocked)
- R206 — Tests exist that assert `unrouted === 0` from worker result and clean status text after routing

## Not Proven By This UAT

- Full WASM-environment pass of R205/R206 — requires CI with WASM binary in Vite serve allow list
- R201, R202, R203 — covered by S01's autoroute-worker.spec.ts, not by this slice's regression tests
- R207 — variant generation via Worker (S04 scope)

## Notes for Tester

- In the current worktree environment, WASM binary path is outside Vite's serving allow list, so all 3 regression tests will skip. This is expected and matches S01's autoroute-worker tests.
- The test count of "109 passed, 8 skipped" is the baseline with both S01 and S03 tests included.
- If testing in a WASM-enabled CI environment, all 3 regression tests should pass (not skip) — this is the true proof of R205/R206.
