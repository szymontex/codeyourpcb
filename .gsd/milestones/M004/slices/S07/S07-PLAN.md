# S07: Benchmark Validation & Strategy Selection

**Goal:** Automated benchmark suite validates all strategies on all fixtures, produces a comparison report, selects the default strategy empirically, and gates regression in CI.

**Demo:** `cargo test benchmark_regression` passes in CI (non-ignored). `cargo test --release --ignored benchmark_full_matrix` routes all 3 fixtures × 2 strategies, prints a comparison table, and emits a JSON report. Playwright E2E captures routed-board screenshots to `test-results/benchmark/`. PathFinder confirmed as default strategy based on empirical composite scores.

## Must-Haves

- Regression gate test (`benchmark_regression`) runs in CI (non-ignored), routes led_blink with PathFinder, asserts composite ≤ baseline × 1.1, DRC ≤ 5, smoothness ≥ 0.95
- Full benchmark matrix test (`benchmark_full_matrix`, `#[ignore]`) iterates all 3 fixtures × 2 strategies, produces comparison table to stderr and JSON report
- Full benchmark asserts PathFinder wins on led_blink (lower composite), confirming default strategy selection
- Playwright E2E test captures canvas screenshots for each benchmark fixture after routing, stored in `test-results/benchmark/`
- Quality gate script updated to invoke the benchmark regression test

## Proof Level

- This slice proves: final-assembly (milestone terminal validation)
- Real runtime required: yes (native Rust routing + WASM browser rendering)
- Human/UAT required: yes (screenshots are artifacts for human visual inspection, not pixel-diffed)

## Verification

- `cargo test -p cypcb-autoroute benchmark_regression --release` — passes, prints score table, asserts regression thresholds
- `cargo test -p cypcb-autoroute --release --ignored -- benchmark_full_matrix` — passes, prints full comparison matrix, emits JSON to stderr
- `cd viewer && npx playwright test benchmark-screenshots` — passes, screenshots written to `test-results/benchmark/`
- `scripts/quality-gate.sh` stage 7 invokes `benchmark_regression` (non-ignored) or the existing `benchmark_500` plus the new regression test
- `cargo test -p cypcb-autoroute benchmark_regression --release 2>&1 | grep -E 'FAIL|threshold|got'` — failure-path messages include actual vs threshold values for diagnostic inspection

## Observability / Diagnostics

- Runtime signals: comparison table printed to stderr with Unicode box-drawing format (per-fixture × per-strategy score breakdown), JSON report for programmatic consumption
- Inspection surfaces: `cargo test benchmark_regression --release -- --nocapture` shows score table; screenshot artifacts at `viewer/test-results/benchmark/*.png`
- Failure visibility: assertion messages include actual vs threshold values with human-readable context (e.g. "composite 5500 exceeds baseline 5001 × 1.1 = 5501")

## Integration Closure

- Upstream surfaces consumed: `parse_kicad_pcb()` from S01, `score_board()` from S02, `PathFinderStrategy`/`ImprovedAStarStrategy` from S03, `smooth_routes()` (integrated into strategies) from S04, `AutorouteParams` from S05, `generate_variants()`/`default_variant_configs()` from S06
- New wiring introduced: benchmark_validation.rs test file, benchmark-screenshots.spec.ts E2E test, quality gate stage 7 update
- What remains before the milestone is truly usable end-to-end: nothing — S07 is the terminal slice

## Tasks

- [x] **T01: Rust benchmark validation suite with regression gate** `est:35m`
  - Why: Core S07 deliverable — creates the automated benchmark pipeline that validates all strategies on all fixtures, confirms PathFinder as default, and gates regression in CI. Covers R114 (benchmark validation) and R116 (empirical strategy selection).
  - Files: `crates/cypcb-autoroute/tests/benchmark_validation.rs`, `scripts/quality-gate.sh`
  - Do: Create `benchmark_validation.rs` with shared helpers (reuse `strategy_comparison.rs` patterns: `fixture_path()`, `test_rules()`, `route_and_score()`). Implement `benchmark_regression` test (non-ignored): routes led_blink with PathFinder only, asserts composite ≤ 5501 (baseline 5001 × 1.1), DRC ≤ 5, smoothness ≥ 0.95, prints score table. Implement `benchmark_full_matrix` test (`#[ignore]`): iterates all 3 BENCHMARKS × 2 strategies (PathFinder, ImprovedAStar), collects results into `Vec<BenchmarkResult>`, prints aggregate comparison table, emits JSON to stderr, asserts PathFinder composite ≤ ImprovedAStar on led_blink, documents strategy selection rationale. Update quality-gate.sh stage 7 to also run `benchmark_regression` (non-ignored test). Use `DesignRules::jlcpcb_2layer()` consistently. Always call `rebuild_spatial_index_with_traces()` before scoring.
  - Verify: `cargo test -p cypcb-autoroute benchmark_regression --release` passes. `cargo test -p cypcb-autoroute --release --ignored -- benchmark_full_matrix` passes and prints comparison table.
  - Done when: Both tests pass in release mode, regression gate asserts within thresholds, full matrix shows PathFinder winning on led_blink

- [x] **T02: Playwright benchmark screenshot E2E tests** `est:20m`
  - Why: Captures visual comparison artifacts for R115 (visual comparison of routed boards). Screenshots are for human inspection, not pixel-diffed.
  - Files: `viewer/e2e/benchmark-screenshots.spec.ts`
  - Do: Create E2E test that loads each benchmark `.kicad_pcb` fixture via `__loadBoard()` (reading fixture content as string), triggers routing via Route button click, waits for completion, captures canvas screenshots to `test-results/benchmark/{fixture_name}.png`. Use the established `beforeEach` pattern from `variant-panel.spec.ts`. Read fixture files using `fs.readFileSync()` in the test (Playwright runs in Node). For each of the 3 benchmark fixtures: load board source → wait for Ready → click Route → wait for routing completion → screenshot canvas. Mark stm32_breakout and multi_ic tests as `test.slow()` since WASM routing may take longer. Screenshots are artifacts, not assertions — test passes if screenshot is captured without page errors.
  - Verify: `cd viewer && npx playwright test benchmark-screenshots` passes, screenshots appear in `test-results/benchmark/`
  - Done when: 3 screenshot files exist in `test-results/benchmark/` after test run, no Playwright test failures

## Files Likely Touched

- `crates/cypcb-autoroute/tests/benchmark_validation.rs` — NEW: regression gate + full matrix benchmark tests
- `viewer/e2e/benchmark-screenshots.spec.ts` — NEW: Playwright screenshot capture E2E
- `scripts/quality-gate.sh` — MODIFIED: stage 7 includes benchmark_regression
