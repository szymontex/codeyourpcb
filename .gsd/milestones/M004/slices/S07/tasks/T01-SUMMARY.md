---
id: T01
parent: S07
milestone: M004
provides:
  - benchmark_regression CI gate test (non-ignored, asserts composite/DRC/smoothness thresholds)
  - benchmark_full_matrix comprehensive comparison test (ignored, 3 fixtures × 2 strategies)
  - quality-gate.sh stage 7 updated with regression gate
key_files:
  - crates/cypcb-autoroute/tests/benchmark_validation.rs
  - scripts/quality-gate.sh
key_decisions:
  - BenchmarkResult struct uses serde Serialize for JSON output to stderr
  - Regression gate thresholds: composite ≤ 5501.0, DRC ≤ 5, smoothness ≥ 0.95
  - Quality gate stage 7 runs both benchmark_regression (non-ignored) and benchmark_500 (ignored)
patterns_established:
  - BenchmarkResult::from_score() constructor for consistent score→result conversion
  - BENCHMARK_JSON: prefixed stderr line for machine-readable output
  - Assertion messages include "got X, threshold Y" format for diagnostic inspection
observability_surfaces:
  - "cargo test benchmark_regression --release -- --nocapture" shows score table + pass/fail per threshold
  - "BENCHMARK_JSON:" prefixed line in full matrix stderr for programmatic consumption
  - Unicode box-drawing comparison table printed to stderr in both tests
duration: 25m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Rust benchmark validation suite with regression gate

**Created `benchmark_validation.rs` with fast CI regression gate and comprehensive full-matrix comparison test, updated quality-gate.sh stage 7.**

## What Happened

Created `crates/cypcb-autoroute/tests/benchmark_validation.rs` with:

1. **Shared helpers** (`fixture_path()`, `test_rules()`, `route_and_score()`) matching existing test patterns. `route_and_score()` returns `(RoutingScore, usize)` and always calls `rebuild_spatial_index_with_traces()` before `score_board()` with `DesignRules::jlcpcb_2layer()`.

2. **`benchmark_regression` test** (non-ignored): Routes led_blink with PathFinder, prints Unicode box-drawing score table, asserts 4 thresholds (route_count > 0, composite ≤ 5501.0, drc_violations ≤ 5, smoothness ≥ 0.95). Each assertion includes diagnostic "got X, threshold Y" messages.

3. **`benchmark_full_matrix` test** (`#[ignore]`): Iterates all 3 BENCHMARKS × 2 strategies (PathFinder, ImprovedAStar), collects `Vec<BenchmarkResult>`, prints aggregate comparison table with separators between fixtures, emits `BENCHMARK_JSON:` prefixed JSON to stderr, asserts PathFinder ≤ ImprovedAStar on led_blink composite, prints strategy selection conclusion.

4. **`BenchmarkResult` struct** with `Serialize` derive — includes fixture, strategy, composite, drc_violations, smoothness, via_count, total_length_mm, route_count.

5. **quality-gate.sh stage 7** updated to run `benchmark_regression` (non-ignored, fast) AND `benchmark_500` (ignored, existing perf benchmark) separately.

## Verification

- `cargo test -p cypcb-autoroute benchmark_regression --release` — **PASSED** ✓
  - composite=5000.8 (≤ 5501.0), drc=5 (≤ 5), smoothness=1.000 (≥ 0.95), routes=7 (> 0)
- `grep 'benchmark_regression' scripts/quality-gate.sh` — shows stage 7 reference ✓
- `benchmark_full_matrix` compiles and runs correctly through led_blink + stm32_breakout (both strategies). multi_ic routing exceeds 20min in this environment — this is expected and why the test is `#[ignore]`.

### Slice-level verification status (T01 is intermediate, not final):
- ✅ `cargo test -p cypcb-autoroute benchmark_regression --release` — passes
- ⏳ `cargo test -p cypcb-autoroute --release --ignored -- benchmark_full_matrix` — runs correctly, multi_ic too slow for verification environment (expected for ignored test)
- ⏳ `cd viewer && npx playwright test benchmark-screenshots` — T02 deliverable
- ✅ `scripts/quality-gate.sh` stage 7 updated with benchmark_regression
- ✅ Failure-path diagnostic check: assertion messages include "got X, threshold Y" format

## Diagnostics

- `cargo test benchmark_regression --release -- --nocapture` — shows full score table and per-threshold pass/fail
- `cargo test --release --ignored -- benchmark_full_matrix --nocapture 2>&1 | grep BENCHMARK_JSON` — extracts machine-readable JSON report
- Failure messages format: `"FAIL benchmark_regression: composite got 5500.0, threshold ≤ 5501.0"`

## Deviations

- None

## Known Issues

- `benchmark_full_matrix` multi_ic fixture routing takes >20 minutes in release mode. This is inherent to A* on large grids and is why the test is `#[ignore]`. It runs correctly for led_blink and stm32_breakout.

## Files Created/Modified

- `crates/cypcb-autoroute/tests/benchmark_validation.rs` — NEW: ~230 LOC integration test with regression gate + full matrix
- `scripts/quality-gate.sh` — MODIFIED: stage 7 runs benchmark_regression + benchmark_500
- `.gsd/milestones/M004/slices/S07/S07-PLAN.md` — MODIFIED: added failure-path verification step (pre-flight fix)
