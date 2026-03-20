---
id: T02
parent: S03
milestone: M005
provides:
  - Verified full E2E suite integration — 109 passed, 8 skipped, 0 failed
  - Confirmed quality gate auto-discovers autoroute-regression.spec.ts without changes
key_files: []
key_decisions: []
patterns_established:
  - Intermittent timing failures in autoroute-worker.spec.ts "overlay visible during worker routing" are a known flake under parallel load — test passes in isolation
observability_surfaces:
  - "Run `cd viewer && npx playwright test --reporter=list` to see both autoroute spec files enumerated"
  - "Run `cd viewer && npx tsc --noEmit` to verify type-level correctness of all E2E tests"
duration: 8m
verification_result: passed
completed_at: 2026-03-19
blocker_discovered: false
---

# T02: Verify full E2E suite integration and quality gate

**Full E2E suite passes (109 passed, 8 skipped, 0 failed) with both autoroute test files auto-discovered; TypeScript clean; quality gate requires no changes**

## What Happened

Ran `cd viewer && npx tsc --noEmit` — exited 0 with zero errors, confirming `autoroute-regression.spec.ts` compiles cleanly alongside all other source files.

Ran `cd viewer && npx playwright test` — first run showed 108 passed, 8 skipped, 1 failed. The single failure was in `autoroute-worker.spec.ts` ("overlay visible during worker routing") at line 67 where `cancelVisible` was false — a timing race where routing completes before the cancel button visibility check fires. Per the plan, this file was not modified. Re-ran the test in isolation: 2 passed, 1 skipped, 0 failures — confirming the flake is not caused by the new regression file. Second full-suite run: 109 passed, 8 skipped, 0 failed.

Verified `scripts/quality-gate.sh` stage 6 runs `(cd viewer && npx playwright test)` which relies on `playwright.config.ts` (`testDir: './e2e'`) to auto-discover all `*.spec.ts` files. The new `autoroute-regression.spec.ts` requires no script changes.

Confirmed `autoroute-worker.spec.ts` is completely unchanged (`git diff --stat` shows no modifications).

## Verification

- `cd viewer && npx tsc --noEmit` — exit 0, zero errors
- `cd viewer && npx playwright test` — exit 0 (retry), 109 passed, 8 skipped, 0 failed
- `cd viewer && npx playwright test e2e/autoroute-worker.spec.ts` — exit 0, 2 passed, 1 skipped (isolation confirms no side effects from new file)
- `scripts/quality-gate.sh` stage 6 auto-discovers both autoroute spec files — no changes needed
- `git diff --stat viewer/e2e/autoroute-worker.spec.ts` — no changes

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cd viewer && npx tsc --noEmit` | 0 | ✅ pass | 36.3s |
| 2 | `cd viewer && npx playwright test --reporter=list` (run 1) | 1 | ⚠️ flake (1 intermittent failure in autoroute-worker.spec.ts) | 35.0s |
| 3 | `cd viewer && npx playwright test e2e/autoroute-worker.spec.ts --reporter=list` (isolation) | 0 | ✅ pass (2 passed, 1 skipped) | 3.4s |
| 4 | `cd viewer && npx playwright test --reporter=list` (run 2) | 0 | ✅ pass (109 passed, 8 skipped) | 34.9s |
| 5 | `git diff --stat viewer/e2e/autoroute-worker.spec.ts` | 0 | ✅ unchanged | <1s |
| 6 | `scripts/quality-gate.sh` stage 6 inspection | — | ✅ auto-discovers (no changes needed) | — |

## Diagnostics

- **Both autoroute test files listed:** `npx playwright test --list` shows 6 autoroute tests (3 from `autoroute-worker.spec.ts`, 3 from `autoroute-regression.spec.ts`).
- **Known flake:** The "overlay visible during worker routing" test in `autoroute-worker.spec.ts` can fail intermittently under parallel worker load when routing completes before the cancel button visibility check. It consistently passes in isolation.
- **Quality gate path:** `scripts/quality-gate.sh` → stage 6 → `(cd viewer && npx playwright test)` → `playwright.config.ts` (`testDir: './e2e'`) → auto-discovers all `e2e/*.spec.ts` files.

## Deviations

None. This was a verification-only task; no source files were created or modified.

## Known Issues

- `autoroute-worker.spec.ts` "overlay visible during worker routing" has an intermittent timing flake under parallel Playwright worker load. This is a pre-existing issue in S01's test, not caused by S03's new regression file. The test design checks cancel button visibility inside a `if (stillActive)` guard, but under heavy load the page state can change between the `stillActive` check and the subsequent DOM query.

## Files Created/Modified

- `.gsd/milestones/M005/slices/S03/tasks/T02-PLAN.md` — Added Observability Impact section (pre-flight fix)
