---
id: T01
parent: S08
milestone: M002
provides:
  - Per-crate opt-level=3 overrides for autorouter speed (32% improvement on blink.cypcb)
  - Adaptive grid resolution scaling for large boards (>80mm → coarser grid)
  - Synthetic 500-component board generator for benchmarking
  - benchmark_500_component test asserting <30s in release mode
key_files:
  - Cargo.toml
  - crates/cypcb-autoroute/src/lib.rs
  - crates/cypcb-autoroute/tests/integration.rs
key_decisions:
  - Adaptive grid doubles resolution at 80mm, triples at 200mm — keeps quality acceptable for larger boards while cutting A* search space
  - Synthetic board uses grid-placed 0805 components with horizontal neighbor net connectivity — realistic enough for benchmarking without needing file I/O
patterns_established:
  - Per-crate Cargo profile overrides for speed-critical crates that aren't in WASM bundle
observability_surfaces:
  - Benchmark tests print timing tables with component count, net count, grid dimensions, routing time, completion percentage
duration: 25m
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Autorouter performance — per-crate opt-level, adaptive grid, 500-component benchmark

**Added per-crate opt-level=3 for cypcb-autoroute and pathfinding, adaptive grid scaling for large boards, and a 500-component benchmark that completes in 0.04s (target: <30s).**

## What Happened

Three changes landed:

1. **Per-crate opt-level=3** in workspace `Cargo.toml` for `cypcb-autoroute` and `pathfinding`. The workspace-wide `opt-level="z"` (size-optimized for WASM) was penalizing autorouter speed. Since cypcb-autoroute is not a WASM dependency, per-crate overrides give us speed without affecting WASM bundle size. Result: blink.cypcb routing dropped from 818ms baseline to 559ms (32% improvement).

2. **Adaptive grid resolution** via `AutorouteConfig::resolve_adaptive_grid_resolution()`. For boards >80mm in either dimension, the grid resolution is doubled (2x coarser cells), reducing cell count by 4x. For boards >200mm, 3x coarser. The `route_board()` entry point now uses adaptive resolution automatically by reading board dimensions before grid construction. blink.cypcb (60x40mm) is below the threshold so existing quality is unchanged.

3. **Synthetic 500-component board generator** in integration tests. `generate_synthetic_board(count)` places components in a grid layout with 3mm pitch using 0805 footprints, connecting horizontal neighbors as nets. The resulting 79x76mm board with 522 nets routes in 0.04s — well within the 30s target. `benchmark_500_component` test is `#[ignore]`-gated like the existing benchmark.

## Verification

All verification checks passed:

- `cargo test --release -p cypcb-autoroute -- benchmark_500 --ignored --nocapture` — **PASS**: 0.04s, 100% completion (522/522 nets), <30s assertion satisfied
- `cargo test --release -p cypcb-autoroute -- benchmark_routing_time --ignored --nocapture` — **PASS**: blink 559ms (improved from 818ms baseline), routing-test 72ms, all complete
- `cargo test -p cypcb-autoroute` — **PASS**: 40 unit + 5 integration tests pass, no regressions
- `cargo test --workspace --exclude cypcb-cli --exclude cypcb-desktop` — **PASS**: all workspace tests pass

### Slice-level verification (partial — T01 of 2):
- ✅ `cargo test --release -p cypcb-autoroute -- benchmark_500_component --ignored --nocapture` — passes with <30s
- ✅ `cargo test --release -p cypcb-autoroute -- benchmark --ignored --nocapture` — existing benchmarks pass
- ⏳ `cd viewer && npx playwright test e2e/performance.spec.ts` — not yet (T02)
- ⏳ `cd viewer && npx jscpd src/ --min-lines 10 --threshold 0` — not yet (T02)
- ⏳ `./scripts/quality-gate.sh` — not yet (T02)

## Diagnostics

- Run `cargo test --release -p cypcb-autoroute -- benchmark --ignored --nocapture` to see timing tables for all boards
- Adaptive grid logs scaling decisions via `tracing::info!` when boards exceed the 80mm threshold
- Benchmark tests print grid dimensions, net counts, and completion percentage for quick diagnosis

## Deviations

- `grid.rs` was not modified — the adaptive resolution logic lives in `lib.rs` (`AutorouteConfig::resolve_adaptive_grid_resolution()`) and is applied in `route_board()` before grid construction. Cleaner than modifying `RoutingGrid::from_board()` since the grid doesn't need to know about adaptivity.
- No `examples/synthetic-500.cypcb` file — the synthetic board is generated in-memory via `generate_synthetic_board()` helper, which is simpler and doesn't require file I/O.

## Known Issues

None.

## Files Created/Modified

- `Cargo.toml` — added `[profile.release.package.cypcb-autoroute]` and `[profile.release.package.pathfinding]` with opt-level=3
- `crates/cypcb-autoroute/src/lib.rs` — added `resolve_adaptive_grid_resolution()` method and updated `route_board()` to use adaptive resolution
- `crates/cypcb-autoroute/tests/integration.rs` — added `generate_synthetic_board()` helper and `benchmark_500_component` test
