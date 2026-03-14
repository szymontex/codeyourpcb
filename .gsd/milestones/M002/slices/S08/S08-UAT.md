# S08: Performance & Polish — UAT

**Milestone:** M002
**Written:** 2026-03-13

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: All targets are quantitative (time, FPS, duplication count) and verified by automated tests — no subjective human judgment needed

## Preconditions

- Rust toolchain installed with release build capability
- Node.js and npm available in `viewer/`
- Playwright browsers installed (`npx playwright install chromium`)

## Smoke Test

Run `./scripts/quality-gate.sh` — all 8 stages pass, exit 0.

## Test Cases

### 1. Autorouter 500-component benchmark

1. `cargo test --release -p cypcb-autoroute -- benchmark_500_component --ignored --nocapture`
2. **Expected:** Test passes, output shows routing time <30s, 100% net completion (522/522)

### 2. Existing autorouter benchmarks unchanged

1. `cargo test --release -p cypcb-autoroute -- benchmark_routing_time --ignored --nocapture`
2. **Expected:** blink.cypcb routes in <1s (baseline 818ms, now ~570ms), routing-test.cypcb routes in <200ms, both 100% completion

### 3. Web load time under 3 seconds

1. `cd viewer && npx playwright test e2e/performance.spec.ts --grep "web load"`
2. **Expected:** domContentLoaded measured <3000ms

### 4. 3D renderer FPS check

1. `cd viewer && npx playwright test e2e/performance.spec.ts --grep "FPS"`
2. **Expected:** `window.__renderer3d.fps` returns ≥30 after 3.5s render period

### 5. Zero code duplication

1. `cd viewer && npx jscpd src/ --min-lines 10 --threshold 0`
2. **Expected:** 0 clones found, 0% duplication

### 6. Full quality gate

1. `./scripts/quality-gate.sh`
2. **Expected:** All 8 stages pass — cargo fmt, clippy, cargo test, eslint, vitest, playwright, autorouter-benchmark, jscpd

## Edge Cases

### Large board adaptive grid

1. The 500-component synthetic board (79x76mm) is just below the 80mm adaptive threshold
2. **Expected:** Grid uses base resolution (no scaling applied), routes in <30s regardless

## Failure Signals

- Quality gate exits non-zero on any stage
- Benchmark test prints routing time >30s
- jscpd reports any clones >10 lines
- Playwright performance tests fail with load time >3000ms or FPS <30

## Requirements Proved By This UAT

- WEB-01 — Web load <3s verified by Navigation Timing API measurement in Playwright

## Not Proven By This UAT

- DESK-10 — Desktop start <1s not automated (desktop crates excluded from quality gate)
- Real-world large board routing quality (only synthetic boards tested)
- Production web deployment load time (tested against local dev server, not CDN)

## Notes for Tester

- Autorouter benchmark is `#[ignore]`-gated — must use `--ignored` flag
- Playwright tests need `npx playwright install chromium` first run
- The 500-component benchmark completing in 0.04s is expected — the router is very efficient for grid-placed simple nets
