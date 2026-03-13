---
estimated_steps: 5
estimated_files: 5
---

# T01: Autorouter performance — per-crate opt-level, adaptive grid, 500-component benchmark

**Slice:** S08 — Performance & Polish
**Milestone:** M002

## Description

The autorouter baseline is ~818ms for blink.cypcb (8 components, 7 nets). Linear extrapolation to 500 components gives ~51s — over the 30s target. Two optimizations bring this within budget: (1) per-crate `opt-level=3` for cypcb-autoroute and pathfinding (currently `opt-level="z"` workspace-wide optimizes for WASM size, not speed), and (2) adaptive grid resolution that uses coarser cells for larger boards (reducing the search space quadratically). A synthetic 500-component test board exercises the target scale.

## Steps

1. **Per-crate opt-level overrides.** Add `[profile.release.package.cypcb-autoroute]` with `opt-level = 3` and `[profile.release.package.pathfinding]` with `opt-level = 3` to workspace `Cargo.toml`. These two crates dominate routing CPU time. The WASM bundle is unaffected because cypcb-autoroute is not a WASM dependency.

2. **Adaptive grid resolution.** Modify `AutorouteConfig::resolve_grid_resolution()` (or add a new method) to scale resolution based on board dimensions. For boards larger than ~80mm in either dimension, double the grid resolution (coarser cells). This reduces grid cell count by 4x, cutting A* search space proportionally. Add a `board_dimensions_nm: Option<(i64, i64)>` parameter or compute from the board in `route_board()`. Verify output quality doesn't degrade unacceptably — check that existing blink.cypcb still routes completely.

3. **Synthetic 500-component board generator.** Create a test helper function `generate_synthetic_board(component_count: usize)` in the integration test file. Strategy: place components in a grid layout on a proportionally-sized board, connect nearest-neighbor pairs as nets (realistic connectivity pattern). Use simple 2-pad footprints (0805-style). This generates a `BoardWorld` directly — no .cypcb file needed.

4. **500-component benchmark test.** Add `benchmark_500_component` test (gated behind `#[ignore]` like the existing benchmark). Route the synthetic board, assert timing <30s, print metrics table. Also verify routing status is Complete or at least >=90% nets routed (some may fail on a synthetic board).

5. **Validate existing benchmarks.** Run the full benchmark suite in release mode to confirm no regression. Existing blink.cypcb routing should be faster with opt-level=3 override.

## Must-Haves

- [ ] Per-crate `opt-level=3` for cypcb-autoroute and pathfinding in workspace Cargo.toml
- [ ] Adaptive grid resolution: boards >80mm get coarser grid
- [ ] Synthetic 500-component board generator producing realistic net connectivity
- [ ] `benchmark_500_component` test asserting <30s in release mode
- [ ] Existing blink.cypcb routing still passes all integration tests

## Verification

- `cargo test --release -p cypcb-autoroute -- benchmark_500 --ignored --nocapture` — passes with <30s timing
- `cargo test --release -p cypcb-autoroute -- benchmark_routing_time --ignored --nocapture` — existing benchmark still runs, timing improved
- `cargo test -p cypcb-autoroute` — all non-ignored tests pass (existing routing quality)
- `cargo test --workspace --exclude cypcb-cli --exclude cypcb-desktop` — no regressions

## Inputs

- `Cargo.toml` — current workspace profile with `opt-level="z"`
- `crates/cypcb-autoroute/src/lib.rs` — `AutorouteConfig` and `route_board()` entry point
- `crates/cypcb-autoroute/src/grid.rs` — `RoutingGrid::from_board()` and grid dimensions
- `crates/cypcb-autoroute/tests/integration.rs` — existing benchmark and routing tests
- S08-RESEARCH.md — baseline measurements and optimization strategy

## Expected Output

- `Cargo.toml` — updated with per-crate release profile overrides
- `crates/cypcb-autoroute/src/lib.rs` — adaptive grid resolution logic
- `crates/cypcb-autoroute/tests/integration.rs` — synthetic board generator + `benchmark_500_component` test
- 500-component routing completes in <30s release, printed timing confirms target met
