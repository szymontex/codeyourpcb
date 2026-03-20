---
estimated_steps: 4
estimated_files: 2
---

# T02: Verify full E2E suite integration and quality gate

**Slice:** S03 — E2E Regression Tests
**Milestone:** M005

## Description

Run the full E2E suite to prove the new `autoroute-regression.spec.ts` coexists with existing tests without regressions. Verify TypeScript compiles cleanly. Confirm the quality gate script auto-discovers the new test file without any changes to `scripts/quality-gate.sh`.

## Steps

1. **TypeScript check.** Run `cd viewer && npx tsc --noEmit`. Must exit 0 with zero errors. If there are errors in `autoroute-regression.spec.ts`, fix them (import paths, type annotations, etc.).

2. **Full E2E suite.** Run `cd viewer && npx playwright test`. This runs all `e2e/*.spec.ts` files including both `autoroute-worker.spec.ts` (3 S01 tests) and `autoroute-regression.spec.ts` (3 new S03 tests). Must exit 0. If any test fails:
   - If failure is in `autoroute-regression.spec.ts` — fix the regression test file
   - If failure is in `autoroute-worker.spec.ts` — do NOT modify it; diagnose whether the new file caused a side effect
   - Never modify production source code

3. **Quality gate verification.** Read `scripts/quality-gate.sh` and confirm stage 6 runs `cd viewer && npx playwright test` which auto-discovers all `e2e/*.spec.ts` files. The new file requires no script changes. Document this confirmation.

4. **Summary.** Report final test counts: total tests, passed, skipped, failed. Confirm both test files are discovered and executed.

## Must-Haves

- [ ] `cd viewer && npx tsc --noEmit` exits 0
- [ ] `cd viewer && npx playwright test` exits 0 with both test files discovered
- [ ] `scripts/quality-gate.sh` requires no modification for new test discovery
- [ ] `autoroute-worker.spec.ts` is unchanged

## Verification

- `cd viewer && npx tsc --noEmit` — exit code 0
- `cd viewer && npx playwright test` — exit code 0, both spec files listed in output

## Inputs

- `viewer/e2e/autoroute-regression.spec.ts` — T01's output, the new regression test file
- `viewer/e2e/autoroute-worker.spec.ts` — existing S01 tests (must remain unmodified)
- `viewer/playwright.config.ts` — testDir: `./e2e`, serial execution, Chromium only
- `scripts/quality-gate.sh` — stage 6 runs `npx playwright test`

## Observability Impact

- **Signals verified (not created):** This task confirms that `autoroute-regression.spec.ts` (3 tests) is auto-discovered alongside `autoroute-worker.spec.ts` by `npx playwright test`. No new runtime signals are introduced.
- **Future agent inspection:** Run `cd viewer && npx playwright test --reporter=list` to see both spec files enumerated with pass/skip/fail status. Run `cd viewer && npx tsc --noEmit` to confirm type-level correctness.
- **Failure state visibility:** If the new regression test file breaks the suite, `npx playwright test` exits non-zero and Playwright outputs the failing test name plus trace zip location in `test-results/`.

## Expected Output

- No files created or modified — this task is verification-only
- If T01's test file needs minor fixes for full-suite compatibility, `viewer/e2e/autoroute-regression.spec.ts` may be patched
