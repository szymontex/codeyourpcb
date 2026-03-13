---
estimated_steps: 4
estimated_files: 4
---

# T05: WASM compilation verification and quality benchmarks

**Slice:** S02 — Custom Autorouter Core
**Milestone:** M002

## Description

Verify the autorouter compiles to `wasm32-unknown-unknown` (hard requirement for web deployment). Add a DRC integration test that proves routed output is design-rule-clean. Establish routing performance benchmarks as baseline for future optimization in S08.

## Steps

1. Verify WASM compilation:
   - Run `cargo build -p cypcb-autoroute --target wasm32-unknown-unknown`
   - Fix any compilation errors (likely: ensure no `std::thread`, `std::fs`, `std::time::Instant` usage in main crate code — use `cfg` guards or feature flags if needed for benchmarks)
   - Ensure `pathfinding` crate compiles for WASM (it's pure Rust, should work)
   - If `tracing` subscriber setup is WASM-incompatible, gate it behind `#[cfg(not(target_arch = "wasm32"))]`

2. Add DRC integration test:
   - In `tests/integration.rs`, add test `routed_output_passes_drc`:
     - Parse `blink.cypcb`, build `BoardWorld`, route with autorouter
     - Apply routes via `apply_routes()`
     - Rebuild spatial index
     - Run DRC checks from `cypcb-drc` on the routed board
     - Assert zero DRC violations (clearance, trace width, via drill, annular ring)
   - This proves the autorouter output respects the design rules it was given

3. Add performance benchmarks:
   - Create `crates/cypcb-autoroute/benches/routing_bench.rs` using Criterion or simple timing
   - Benchmark: time to route `routing-test.cypcb` (3 components)
   - Benchmark: time to route `blink.cypcb` (8 components)
   - Benchmark: grid construction time for a 100×100mm board
   - Record baseline numbers in test output / comments for S08 comparison
   - If Criterion is not in workspace, use `std::time::Instant` in a regular test with `#[ignore]` tag for manual benchmarking (guard with `#[cfg(not(target_arch = "wasm32"))]`)

4. Final cleanup and verification:
   - Run `cargo clippy -p cypcb-autoroute -- -D warnings` — fix any warnings
   - Run `cargo test -p cypcb-autoroute` — all tests green
   - Run `cargo build -p cypcb-autoroute --target wasm32-unknown-unknown` — compiles
   - Verify `tracing` spans are present: run tests with `RUST_LOG=cypcb_autoroute=debug` and confirm structured output
   - Verify all public API has doc comments

## Must-Haves

- [ ] `cargo build -p cypcb-autoroute --target wasm32-unknown-unknown` compiles successfully
- [ ] DRC integration test passes — zero violations on routed `blink.cypcb`
- [ ] Performance baseline recorded for both reference boards
- [ ] `cargo clippy -p cypcb-autoroute -- -D warnings` passes
- [ ] All tests pass (`cargo test -p cypcb-autoroute`)

## Verification

- `cargo build -p cypcb-autoroute --target wasm32-unknown-unknown` — success
- `cargo test -p cypcb-autoroute` — all tests pass including DRC integration
- `cargo clippy -p cypcb-autoroute -- -D warnings` — zero warnings
- Performance benchmark test runs and prints timing

## Observability Impact

- Signals added/changed: None new — this task validates existing observability works end-to-end
- How a future agent inspects this: benchmark timing printed in test output; DRC test provides concrete pass/fail on design rule compliance
- Failure state exposed: DRC violations listed with violation type, location, and actual vs required values

## Inputs

- `crates/cypcb-autoroute/` — complete autorouter from T01-T04
- `crates/cypcb-drc/` — DRC engine for post-routing validation
- `crates/cypcb-router/src/lib.rs` — `apply_routes()` for applying routing to board

## Expected Output

- `crates/cypcb-autoroute/Cargo.toml` — potentially updated with dev-dependencies for DRC
- `crates/cypcb-autoroute/tests/integration.rs` — DRC integration test added
- `crates/cypcb-autoroute/benches/routing_bench.rs` — performance benchmarks (or `#[ignore]` timing test)
- WASM compilation verified — no source changes needed if plan was followed correctly
