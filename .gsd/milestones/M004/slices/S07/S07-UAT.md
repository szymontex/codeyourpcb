# S07: Benchmark Validation & Strategy Selection — UAT

**Milestone:** M004
**Written:** 2026-03-14

## UAT Type

- UAT mode: mixed (artifact-driven + live-runtime)
- Why this mode is sufficient: Rust benchmark tests run real routing algorithms on real fixtures (live-runtime). Playwright screenshots are artifacts for human visual inspection (artifact-driven). Both are needed for terminal milestone validation.

## Preconditions

- Rust toolchain available with `--release` profile
- `cargo test` compiles cypcb-autoroute and its integration tests
- Node.js + Playwright installed in `viewer/` (with `npx playwright install`)
- Vite dev server can start in `viewer/` (for Playwright tests)
- Benchmark fixtures exist at `tests/fixtures/benchmark/` (led_blink.kicad_pcb, stm32_breakout.kicad_pcb, multi_ic.kicad_pcb)

## Smoke Test

Run `cargo test -p cypcb-autoroute benchmark_regression --release` — must pass in <30s with all thresholds met. This single command proves the entire autorouter pipeline works: KiCad parsing → routing → scoring → threshold assertion.

## Test Cases

### 1. Regression Gate — Threshold Assertions

1. Run `cargo test -p cypcb-autoroute benchmark_regression --release -- --nocapture`
2. Observe the Unicode score table printed to stderr
3. **Expected:** Test passes. Output shows:
   - `✓ route_count: got 7, threshold > 0`
   - `✓ composite: got ≈5001, threshold ≤ 5501.0`
   - `✓ drc_violations: got ≤5, threshold ≤ 5`
   - `✓ smoothness: got ≥0.95, threshold ≥ 0.95`
   - `═══ benchmark_regression PASSED ═══` summary line

### 2. Regression Gate — Failure Diagnostics

1. Run `cargo test -p cypcb-autoroute benchmark_regression --release 2>&1 | grep -E 'FAIL|threshold|got'`
2. **Expected:** No lines matching "FAIL" appear (test passes, so only "✓" lines with "got" and "threshold" are visible). If the test were failing, messages like `"FAIL benchmark_regression: composite got X, threshold ≤ Y"` would appear.

### 3. Full Matrix Comparison (Manual Run)

1. Run `cargo test -p cypcb-autoroute --release --ignored -- benchmark_full_matrix --nocapture`
2. Observe the comparison table printed to stderr (3 fixtures × 2 strategies)
3. **Expected:**
   - Led_blink rows show PathFinder composite < ImprovedAStar composite
   - Table uses Unicode box-drawing characters with separator lines between fixtures
   - A `BENCHMARK_JSON:` prefixed line appears in stderr with valid JSON array
   - Test asserts PathFinder wins on led_blink and prints strategy selection conclusion
4. Note: multi_ic may take >20min — you can Ctrl+C after led_blink and stm32_breakout rows appear

### 4. JSON Report Extraction

1. Run `cargo test -p cypcb-autoroute --release --ignored -- benchmark_full_matrix --nocapture 2>&1 | grep BENCHMARK_JSON | sed 's/BENCHMARK_JSON://'`
2. Pipe through `jq .` (or paste into JSON validator)
3. **Expected:** Valid JSON array of BenchmarkResult objects, each with fields: fixture, strategy, composite, drc_violations, smoothness, via_count, total_length_mm, route_count

### 5. Playwright Screenshot Capture

1. Run `cd viewer && npx playwright test benchmark-screenshots --reporter=list`
2. **Expected:** 3 tests pass:
   - `capture routed board: led_blink`
   - `capture routed board: stm32_breakout`
   - `capture routed board: multi_ic`
3. Check `viewer/test-results/benchmark/` directory
4. **Expected:** 6 PNG files exist:
   - `led_blink.png`, `led_blink-canvas.png`
   - `stm32_breakout.png`, `stm32_breakout-canvas.png`
   - `multi_ic.png`, `multi_ic-canvas.png`

### 6. Screenshot Visual Inspection

1. Open each `*-canvas.png` file in an image viewer
2. **Expected:**
   - `led_blink-canvas.png` — shows a small board with ~7 components, routed traces visible, no blank/white canvas
   - `stm32_breakout-canvas.png` — shows a medium board with ~29 components, routed traces visible
   - `multi_ic-canvas.png` — shows a larger board with ~52 components, routed traces visible
3. File sizes should be >10KB (indicates rendered content, not blank captures)

### 7. Quality Gate Stage 7

1. Inspect `scripts/quality-gate.sh` for stage 7 content
2. **Expected:** Stage 7 runs `cargo test --release -p cypcb-autoroute -- benchmark_regression` as a non-ignored test
3. **Expected:** Stage 7 also runs `benchmark_500` as an `--ignored` test

## Edge Cases

### Multi-IC Routing Timeout
1. Run `cargo test -p cypcb-autoroute --release --ignored -- benchmark_full_matrix --nocapture` and observe multi_ic fixture
2. **Expected:** multi_ic routing takes >10 minutes for each strategy (A* on large grids). This is why the test is `#[ignore]`. The test is correct; the algorithm is slow on large boards. Led_blink and stm32_breakout complete within reason.

### Screenshot File Sizes
1. After running Playwright tests, check: `ls -la viewer/test-results/benchmark/`
2. **Expected:** All files >5KB. If any file is <5KB, it may indicate the board didn't load or routing failed silently.

### Regression Threshold Sensitivity
1. The composite threshold is 5501.0 (baseline 5001 × 1.1)
2. If the routing algorithm changes and composite rises above 5501, the benchmark_regression test will fail with a clear diagnostic message showing actual vs threshold
3. **Expected:** This is working as designed — the regression gate prevents silent quality degradation

## Failure Signals

- `benchmark_regression` test fails → routing quality has degraded; check "got X, threshold Y" in output
- Playwright screenshot test fails → WASM or viewer pipeline broken; check browser console errors
- Screenshot files are blank/tiny (<5KB) → board loading or rendering broken
- `BENCHMARK_JSON` line missing from full matrix output → BenchmarkResult serialization broken
- quality-gate.sh stage 7 doesn't run benchmark_regression → script update lost

## Requirements Proved By This UAT

- R114 — Benchmark validation: Test cases 1-4 prove automated benchmark pipeline routes fixtures, compares strategies, produces reports
- R115 — Visual comparison: Test cases 5-6 prove screenshot capture and visual artifact generation
- R116 — Empirical strategy selection: Test case 3 proves PathFinder wins empirically on led_blink, confirming default strategy

## Not Proven By This UAT

- Zero DRC violations (R107) — DRC is 5 on led_blink, not zero. Regression gate accepts ≤ 5.
- Real-project benchmarks (R102 partial) — fixtures are synthetic KiCad 8 files, not downloaded real projects
- Sub-second WASM routing for all boards — WASM performance tested elsewhere (S05/S06), not specifically in benchmark context
- Pixel-perfect rendering comparison — screenshots are for human review, no automated visual diff

## Notes for Tester

- The `benchmark_full_matrix` test is slow (~20min+ for multi_ic). Run with `--nocapture` to see progress. You can verify led_blink + stm32_breakout rows and Ctrl+C before multi_ic completes — the led_blink assertion is the critical one.
- Screenshot artifacts are overwritten on each Playwright run. Save copies if you want to compare across changes.
- The regression test runs PathFinder only (not ImprovedAStar) — it's the CI gate, optimized for speed. Full comparison is the `benchmark_full_matrix` ignored test.
- Composite score ≈5001 is the current baseline. If you see a significantly lower number, that's an improvement. If above 5501, the gate trips.
