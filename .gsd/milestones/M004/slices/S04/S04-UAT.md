# S04: Trace Smoother & Via Optimizer — UAT

**Milestone:** M004
**Written:** 2026-03-14

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: Slice is pure Rust algorithmic work (smoother + via optimizer) with no UI or runtime components. All verification is through cargo test output, score metrics, and WASM compilation. No live server or human-experience testing needed.

## Preconditions

- Rust toolchain installed with `wasm32-unknown-unknown` target
- Repository checked out at S04-complete state
- `cargo build --release` succeeds without errors
- KiCad benchmark fixture `led_blink.kicad_pcb` exists in `tests/fixtures/benchmark/`

## Smoke Test

Run `cargo test --test smoother_integration --release` — should pass in <30s with smoothness ≥ 0.5 and DRC violations ≤ 5.

## Test Cases

### 1. Unit tests pass with all smoother/via_optimizer tests

1. Run `cargo test -p cypcb-autoroute --lib --release`
2. Count tests: should report 110 passed, 0 failed
3. **Expected:** All 110 tests pass including 17 smoother tests and 5 via_optimizer tests

### 2. Staircase collapse produces clean diagonals

1. Run `cargo test -p cypcb-autoroute --lib --release -- staircase_collapse_10`
2. **Expected:** Test passes — a 10-step H/V staircase collapses to ≤3 output segments (single diagonal + orthogonal tails)

### 3. Corner chamfering inserts 45° segments

1. Run `cargo test -p cypcb-autoroute --lib --release -- chamfer_90_degree_bend`
2. **Expected:** A 90° bend gets a 45° chamfer segment inserted between the two legs. Output has 3 segments where input had 2.

### 4. Collinear segments merge

1. Run `cargo test -p cypcb-autoroute --lib --release -- merge_collinear`
2. **Expected:** Two consecutive same-direction segments merge into one. Output segment count < input count.

### 5. DRC rejection preserves original segments

1. Run `cargo test -p cypcb-autoroute --lib --release -- drc_rejection_staircase_blocked`
2. **Expected:** When an obstacle blocks the diagonal shortcut, the original staircase is preserved (DRC safety). No segments violate clearance.

### 6. Via pair elimination works when path is clean

1. Run `cargo test -p cypcb-autoroute --lib --release -- via_pair_eliminated`
2. **Expected:** Two complementary vias (L1→L2 + L2→L1) with a short between-segment are eliminated when a direct path on L1 is DRC-clean. Via count goes from 2 to 0.

### 7. Via pair preserved when obstacle blocks direct path

1. Run `cargo test -p cypcb-autoroute --lib --release -- via_pair_not_eliminated`
2. **Expected:** Vias kept when an obstacle on the original layer blocks the direct path. Via count unchanged.

### 8. Integration test proves smoothness improvement on led_blink

1. Run `cargo test --test smoother_integration --release -- --nocapture`
2. Read the score table printed to stderr
3. **Expected:** smoothness = 1.000, DRC violations ≤ 5, vias = 0, composite score ≈ 5000.8

### 9. Strategy comparison still holds with smoother active

1. Run `cargo test --test strategy_comparison --release -- led_blink`
2. **Expected:** PathFinder composite ≤ ImprovedAStar composite (PathFinder still wins)

### 10. WASM compilation succeeds

1. Run `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown`
2. **Expected:** Clean compilation — no errors, no use of std::time, std::fs, or other non-WASM APIs in smoother/via_optimizer modules

### 11. Observability diagnostics fire correctly

1. Run `RUST_LOG=cypcb_autoroute=info cargo test --test smoother_integration --release -- --nocapture`
2. Scan stderr for tracing output
3. **Expected:** See `smooth_routes` info logs with before/after segment counts. Score table prints with all 5 metrics.

## Edge Cases

### Empty input handling

1. Run `cargo test -p cypcb-autoroute --lib --release -- empty_input`
2. **Expected:** `smooth_routes([])` returns empty vec. No panic, no error.

### Single segment (nothing to smooth)

1. Run `cargo test -p cypcb-autoroute --lib --release -- single_segment`
2. **Expected:** Single segment passes through unchanged. No chamfering or merging attempted.

### Zero-length segment

1. Run `cargo test -p cypcb-autoroute --lib --release -- zero_length_segment`
2. **Expected:** Zero-length segments are handled without division-by-zero or panic.

### Segment metadata preservation

1. Run `cargo test -p cypcb-autoroute --lib --release -- net_id_layer_width_preserved`
2. **Expected:** After smoothing, all output segments retain the correct net_id, layer, and width from input segments.

### Already-smooth geometry (45° input)

1. Run `cargo test -p cypcb-autoroute --lib --release -- chamfer_already_45_no_op`
2. **Expected:** A segment pair already at 45° angles is not modified by chamfering. Output matches input.

## Failure Signals

- Any test in `cargo test -p cypcb-autoroute --lib --release` fails → core algorithm broken
- `smoother_integration` test fails on smoothness < 0.5 → smoother not effective on real routing output
- `smoother_integration` test fails on DRC violations > 5 → smoother introduced new DRC violations (safety regression)
- WASM check fails → non-WASM-compatible code introduced in smoother/via_optimizer
- Strategy comparison flips (ImprovedAStar beats PathFinder) → smoother integration may have broken PathFinder output

## Requirements Proved By This UAT

- R108 (Clean 45°/90° Trace Geometry) — Test case 8 proves smoothness=1.000; test cases 2-4 prove staircase/chamfer/merge mechanics
- R109 (Trace Smoothing Post-Processor) — Test cases 2-8 prove full pipeline works end-to-end with DRC safety
- R107 (Zero DRC Violations) — Test case 5 proves DRC rejection path works; test case 8 proves non-regression (violations ≤ 5)

## Not Proven By This UAT

- R107 target of zero DRC violations — still at 5, grid artifacts from S03. Target S07.
- Visual quality of smoothed traces — no screenshot comparison (deferred to S07)
- Smoother effect on complex boards (stm32_breakout, multi_ic) — only led_blink tested in CI, larger boards are #[ignore] tests
- Realtime performance of smoother in WASM — no timing benchmark (S05 concern)

## Notes for Tester

- Integration test takes ~22s in release mode due to PathFinder routing (not smoother itself)
- DRC violation count of 5 is the S03 baseline — smoother does not reduce it (those violations are grid artifacts)
- Via optimizer has zero effect on led_blink because PathFinder already produces 0 vias — tested via unit tests with synthetic scenarios instead
- Run with `RUST_LOG=cypcb_autoroute::smoother=debug` for detailed per-move logs during any test
