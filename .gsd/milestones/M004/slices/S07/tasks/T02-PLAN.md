---
estimated_steps: 4
estimated_files: 1
---

# T02: Playwright benchmark screenshot E2E tests

**Slice:** S07 — Benchmark Validation & Strategy Selection
**Milestone:** M004

## Description

Create a Playwright E2E test that loads each benchmark KiCad fixture, triggers routing, and captures canvas screenshots as visual comparison artifacts. Screenshots are stored in `test-results/benchmark/` for human inspection — not pixel-diffed. This delivers R115 (visual comparison of routed boards).

## Steps

1. Create `viewer/e2e/benchmark-screenshots.spec.ts`. Import `test`, `expect` from `@playwright/test` and `fs`, `path` from Node built-ins. Define a helper that reads a `.kicad_pcb` fixture file and returns its content as a string (use `fs.readFileSync` with the path resolved relative to the workspace root via `path.resolve(__dirname, '../../tests/fixtures/benchmark/', filename)`).

2. Implement a `test.describe('Benchmark Screenshots')` block. For each of the 3 fixtures (`led_blink.kicad_pcb`, `stm32_breakout.kicad_pcb`, `multi_ic.kicad_pcb`), create a test that:
   - Navigates to `/`, waits for Ready status
   - Reads the fixture file content
   - Calls `window.__loadBoard(source)` with the fixture content as source string
   - Waits for board to render (short delay or network_idle)
   - Clicks the Route button (`#route-btn`)
   - Waits for routing to complete (wait for status text change or 5s timeout)
   - Captures a full-page screenshot to `test-results/benchmark/{fixture_name}.png`
   - Also captures a canvas-only screenshot to `test-results/benchmark/{fixture_name}-canvas.png`

3. Mark `stm32_breakout` and `multi_ic` tests as `test.slow()` to allow extra time for WASM routing on complex boards. Use `test.setTimeout(60_000)` for those fixtures.

4. Ensure `test-results/benchmark/` directory is created (Playwright's `page.screenshot({ path })` creates parent dirs). Test assertion is simply that no page errors occurred — screenshots are artifacts for human review.

## Must-Haves

- [ ] Test loads all 3 benchmark `.kicad_pcb` fixtures via `__loadBoard()`
- [ ] Route button triggered for each fixture
- [ ] Canvas screenshots captured to `test-results/benchmark/`
- [ ] stm32_breakout and multi_ic tests have extended timeout
- [ ] Tests pass without assertions on screenshot content (artifact-only)

## Verification

- `cd viewer && npx playwright test benchmark-screenshots --reporter=list` — all tests pass
- `ls viewer/test-results/benchmark/` — shows `led_blink.png`, `stm32_breakout.png`, `multi_ic.png` (or subset if complex boards timeout in mock mode)

## Inputs

- `viewer/e2e/variant-panel.spec.ts` — pattern for `__loadBoard()` usage and Route button interaction
- `viewer/e2e/app-load.spec.ts` line 51 — pattern for `page.screenshot({ path })` 
- `tests/fixtures/benchmark/` — the 3 `.kicad_pcb` files to load
- `viewer/playwright.config.ts` — webServer config, base URL, test directory

## Expected Output

- `viewer/e2e/benchmark-screenshots.spec.ts` — NEW: ~80 LOC Playwright E2E test
- `viewer/test-results/benchmark/*.png` — screenshot artifacts (generated at test runtime, not committed)

## Observability Impact

- **Artifacts:** Routed-board screenshots written to `viewer/test-results/benchmark/{fixture}-canvas.png` and `viewer/test-results/benchmark/{fixture}.png` — visual comparison artifacts for human review of routing quality across fixtures
- **Inspection command:** `ls -la viewer/test-results/benchmark/` — confirms screenshots were generated; file sizes indicate whether routing produced meaningful output (>10KB = routed, <5KB = blank/error)
- **Console diagnostics:** Page errors are collected during each test run; any JS errors during fixture loading or routing surface as test failures with the error message in Playwright's reporter output
- **Failure visibility:** If `__loadBoard()` or routing fails, the screenshot still captures the error state visible in the UI (status bar text), making failure mode diagnosable from artifacts alone
- **Future agent inspection:** Run `npx playwright test benchmark-screenshots --reporter=list` from `viewer/`; check exit code for pass/fail, check `test-results/benchmark/` for screenshot artifacts
