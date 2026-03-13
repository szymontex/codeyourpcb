---
id: T01
parent: S02
milestone: M002
provides:
  - cypcb-autoroute crate scaffold with grid model
  - RoutingGrid coordinate conversion and occupancy tracking
  - Integration test harness for reference boards
key_files:
  - crates/cypcb-autoroute/Cargo.toml
  - crates/cypcb-autoroute/src/lib.rs
  - crates/cypcb-autoroute/src/grid.rs
  - crates/cypcb-autoroute/tests/integration.rs
key_decisions:
  - Grid uses flat Vec<u8> per layer (not 2D Vec) for cache-friendly access
  - Cell occupancy is a bitfield (pad|trace|zone|via|obstacle) supporting multiple overlapping flags
  - Clearance bloat applied as circular radius around obstacles (pad radius + clearance cells)
  - Layer indexing: TopCopper=0, BottomCopper=1, Inner(n)=2+n
  - Net ownership tracked separately in net_map for rip-up-and-retry support
patterns_established:
  - Integration tests use workspace_path() helper via CARGO_MANIFEST_DIR to resolve example files
  - parse_board() helper wraps parse + sync_ast_to_world for test convenience
  - Grid construction takes &mut BoardWorld (required by bevy_ecs query API)
observability_surfaces:
  - tracing info_span "grid_construction" logs board dimensions, grid dimensions, resolution
  - tracing warn when grid exceeds 10M cells
  - GridStats struct exposes width, height, layers, obstacle_cell_count, resolution_nm
duration: 25min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Scaffold cypcb-autoroute crate with grid model and integration test harness

**Created cypcb-autoroute crate with RoutingGrid (coordinate mapping, per-layer occupancy, obstacle population from BoardWorld) and integration test harness against reference boards.**

## What Happened

Built the `cypcb-autoroute` crate as a new workspace member with:

1. **`AutorouteConfig`** — configurable grid resolution, max rip-up iterations, via cost multiplier
2. **`route_board()` stub** — returns `RoutingResult::failed("autorouter not yet implemented")`
3. **`RoutingGrid`** — full grid data structure:
   - Coordinate conversion (`nm_to_grid` / `grid_to_nm`) with board origin offset
   - Per-layer occupancy tracking using `Vec<Vec<u8>>` bitfield per cell
   - `from_board()` populates obstacles from pads (via footprint library), keepout zones, and locked traces
   - Circular clearance bloat around obstacles based on design rules
   - `mark_route()` / `clear_route()` for dynamic net tracking (rip-up support)
   - `stats()` returns `GridStats` for observability
4. **12 unit tests** covering coordinate round-trips, obstacle marking, clearance bloat, layer isolation, route marking/clearing
5. **3 integration tests**: `grid_from_blink` (passes), `route_routing_test_board` and `route_blink_board` (fail on stub as expected)

## Verification

- `cargo build -p cypcb-autoroute` — compiles without errors ✅
- `cargo test -p cypcb-autoroute --lib` — 12/12 unit tests pass ✅
- `cargo test -p cypcb-autoroute -- grid_from_blink` — passes, grid dimensions match board (945×630 cells, 2 layers, 11968 obstacle cells) ✅
- `cargo test -p cypcb-autoroute -- route_routing_test` — compiles, fails on stub with "autorouter not yet implemented" ✅ (expected)
- `cargo clippy -p cypcb-autoroute -- -W clippy::all` — zero warnings from our crate ✅
- No `std::thread` / `std::fs` in main crate source — WASM compatible ✅
- WASM build: `cargo build --target wasm32-unknown-unknown` fails on transitive `getrandom` dependency (pre-existing workspace issue, not introduced by this crate)

### Slice-level verification status (T01 of multi-task slice):
- `cargo test -p cypcb-autoroute` — unit tests pass, grid integration passes, routing tests fail on stub (expected) ⏳
- `cargo clippy -p cypcb-autoroute -- -D warnings` — pre-existing upstream clippy issue in cypcb-core, our crate is clean ⏳
- `cargo build -p cypcb-autoroute --target wasm32-unknown-unknown` — pre-existing getrandom issue ⏳
- Integration routing assertions — blocked until router is implemented in T02+ ⏳

## Diagnostics

- Run `cargo test -p cypcb-autoroute -- grid_from_blink -- --nocapture` to see grid stats printed
- `RoutingGrid::stats()` returns `GridStats` with width, height, layers, obstacle_cell_count
- Grid construction emits tracing spans and warnings — enable with `RUST_LOG=cypcb_autoroute=info`

## Deviations

- Task plan mentioned `pathfinding` and `rstar` dependencies — not added yet since the grid module doesn't need them. They'll be added when A* pathfinding is implemented in T02.
- Task plan specified `Vec<Vec<u8>>` for occupancy which implies 2D; implemented as flat `Vec<u8>` per layer indexed by `y * width + x` for better cache performance.
- `route_board()` takes `&mut BoardWorld` instead of `&BoardWorld` because bevy_ecs query API requires mutable world reference.

## Known Issues

- WASM target fails to compile due to pre-existing `getrandom` transitive dependency issue (not introduced by this task)
- Copper pour zones are excluded from obstacles (they fill around traces); this may need revisiting if pour-aware routing is needed

## Files Created/Modified

- `crates/cypcb-autoroute/Cargo.toml` — new crate manifest with dependencies
- `crates/cypcb-autoroute/src/lib.rs` — module structure, AutorouteConfig, route_board() stub
- `crates/cypcb-autoroute/src/grid.rs` — RoutingGrid with coordinate mapping, occupancy tracking, obstacle population from BoardWorld
- `crates/cypcb-autoroute/tests/integration.rs` — integration test harness with 3 tests against reference boards
- `Cargo.toml` — added cypcb-autoroute to workspace dependencies
