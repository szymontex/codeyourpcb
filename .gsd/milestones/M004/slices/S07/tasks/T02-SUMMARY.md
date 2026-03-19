---
id: T02
parent: S07
milestone: M004
provides:
  - Playwright E2E benchmark screenshot test capturing routed-board visuals for all 3 fixtures
  - Screenshot artifacts at viewer/test-results/benchmark/ for human visual comparison (R115)
key_files:
  - viewer/e2e/benchmark-screenshots.spec.ts
key_decisions:
  - Used import.meta.url ESM pattern for __dirname (consistent with existing E2E tests like renderer-quality.spec.ts)
  - waitForFunction on status text change to detect routing completion (handles mock, WASM, and error states)
  - Canvas-only screenshots captured via locator.screenshot() in addition to full-page screenshots
patterns_established:
  - Benchmark screenshot test pattern: readFixture helper → __loadBoard() → Route click → waitForFunction on status → screenshot
observability_surfaces:
  - Screenshot artifacts at viewer/test-results/benchmark/{fixture}.png and {fixture}-canvas.png
  - File sizes indicate routing success (>10KB = rendered content)
  - Page errors collected and asserted empty — any JS errors during load/route surface as test failures
duration: 8m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: Playwright benchmark screenshot E2E tests

**Created `benchmark-screenshots.spec.ts` — E2E test loads all 3 KiCad benchmark fixtures, triggers routing, and captures canvas + full-page screenshots to `test-results/benchmark/`.**

## What Happened

Created `viewer/e2e/benchmark-screenshots.spec.ts` (~80 LOC) implementing a `Benchmark Screenshots` test suite that iterates over the 3 benchmark fixtures (`led_blink`, `stm32_breakout`, `multi_ic`). Each test:

1. Navigates to `/`, waits for WASM Ready status
2. Reads the `.kicad_pcb` fixture via `fs.readFileSync` using ESM-compatible `__dirname`
3. Loads the board via `window.__loadBoard(source)`
4. Clicks `#route-btn` to trigger routing
5. Waits for routing completion via `waitForFunction` on status text changes
6. Captures full-page screenshot and canvas-only screenshot to `test-results/benchmark/`

`stm32_breakout` and `multi_ic` tests are marked `test.slow()` with 60s timeout. Page errors are collected and asserted empty — screenshots are artifacts for human review, not pixel-diffed.

## Verification

- `cd viewer && npx playwright test benchmark-screenshots --reporter=list` — **3 passed** (13.5s)
- `ls viewer/test-results/benchmark/` — 6 files: `led_blink.png`, `led_blink-canvas.png`, `stm32_breakout.png`, `stm32_breakout-canvas.png`, `multi_ic.png`, `multi_ic-canvas.png`
- Slice-level: `cargo test -p cypcb-autoroute --test benchmark_validation benchmark_regression --release` — **passed**, prints score table
- Slice-level: `cd viewer && npx playwright test benchmark-screenshots` — **passed**

### Slice verification status (T02 is final task):
- ✅ `cargo test -p cypcb-autoroute benchmark_regression --release` — passed
- ⏭️ `cargo test -p cypcb-autoroute --release --ignored -- benchmark_full_matrix` — not re-run (T01 verified, no changes to Rust code)
- ✅ `cd viewer && npx playwright test benchmark-screenshots` — passed
- ⏭️ `scripts/quality-gate.sh` stage 7 — updated in T01, no changes needed
- ✅ `cargo test benchmark_regression --release 2>&1 | grep -E 'threshold|got'` — shows actual vs threshold values

## Diagnostics

- Run `cd viewer && npx playwright test benchmark-screenshots --reporter=list` to re-verify
- Check `viewer/test-results/benchmark/` for screenshot artifacts after test run
- File sizes >10KB indicate routing produced visible content; <5KB would suggest blank/error state
- Playwright trace files retained on failure for debugging

## Deviations

- Used `import.meta.url` / `fileURLToPath` pattern instead of bare `__dirname` — required because Playwright config uses ESM modules (discovered at runtime, consistent with existing tests)

## Known Issues

None.

## Files Created/Modified

- `viewer/e2e/benchmark-screenshots.spec.ts` — NEW: Playwright E2E test for benchmark screenshot capture (~80 LOC)
- `.gsd/milestones/M004/slices/S07/tasks/T02-PLAN.md` — MODIFIED: Added Observability Impact section
