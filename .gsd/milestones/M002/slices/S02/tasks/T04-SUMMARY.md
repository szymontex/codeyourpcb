---
id: T04
parent: S02
milestone: M002
provides:
  - postprocess.rs module with path simplification (collinear merge) and output conversion
  - Full blink.cypcb validation with quality assertions and apply_routes() compatibility
  - Integration test for output contract — traces and vias correctly spawned as ECS entities
key_files:
  - crates/cypcb-autoroute/src/postprocess.rs
  - crates/cypcb-autoroute/tests/integration.rs
  - crates/cypcb-autoroute/src/lib.rs
key_decisions:
  - Extracted path post-processing from orchestrator.rs into dedicated postprocess.rs module rather than keeping it inline — cleaner separation, easier to test
  - Introduced intermediate types (PathSegment, LayerTransition, SimplifiedPath) for the simplification pipeline — makes the data flow explicit between grid paths and RouteSegments
patterns_established:
  - simplify_path() -> convert_to_route_segments() pipeline with intermediate types, wrapped by paths_to_output() convenience function
  - Integration tests scope ECS queries in blocks to avoid Hecs borrow conflicts
observability_surfaces:
  - tracing::info! in paths_to_output() logs per-net post-processing stats (raw_steps, segments, vias)
  - Integration test prints metrics table with box-drawing characters for visual comparison
  - RoutingMetrics attached to RoutingResult for programmatic inspection
duration: 30min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T04: Path post-processing, output conversion, and blink.cypcb validation

**Created postprocess.rs with collinear segment merging and coordinate conversion; validated blink.cypcb routes 7/7 nets with JLCPCB-correct trace widths and apply_routes() compatibility.**

## What Happened

Extracted the path simplification and output conversion logic from `orchestrator.rs` into a dedicated `postprocess.rs` module with clean intermediate types (`PathSegment`, `LayerTransition`, `SimplifiedPath`). The module provides:

- `simplify_path()` — detects direction changes and merges collinear grid steps into minimal straight segments
- `convert_to_route_segments()` — maps grid coordinates to Nm, assigns trace width from rules, creates ViaPlacement for layer transitions
- `paths_to_output()` — convenience wrapper that processes all paths for a net and logs stats

Updated `route_board()` in `lib.rs` to use the new `postprocess::paths_to_output()` instead of the old inline `paths_to_segments()`. Removed the duplicate code from `orchestrator.rs`.

Enhanced integration tests with comprehensive validation:
- All segments match JLCPCB min_trace_width (0.127mm) and min_via_drill (0.3mm)
- All layers are TopCopper or BottomCopper (correct for 2-layer board)
- Quality bounds: 182.5mm total length (< 500mm bound), 8 vias (< 20 bound)
- `apply_routes()` compatibility test verifies ECS entities are spawned correctly

## Verification

- `cargo test -p cypcb-autoroute` — 44 tests pass (40 unit + 4 integration)
- `cargo clippy -p cypcb-autoroute -- -D warnings` — zero warnings from cypcb-autoroute
- blink.cypcb: Complete, 46 segments, 8 vias, 182.5mm total, quality score 222.5
- routing-test.cypcb: Complete, all segments valid
- apply_routes: Trace and Via entities spawned correctly, segment counts match

Slice-level verification:
- ✅ `cargo test -p cypcb-autoroute` — all pass
- ✅ Integration tests route both reference boards to completion
- ✅ clippy clean (upstream deps have pre-existing issues, not ours)
- ⚠️ WASM compile fails due to `getrandom` crate dependency — pre-existing, T05 scope
- ✅ All route segments have valid width matching rule constraints
- ✅ Quality metrics within bounds

## Diagnostics

- Run `RUST_LOG=cypcb_autoroute=info cargo test -p cypcb-autoroute -- route_blink --nocapture` to see per-net post-processing stats
- Integration test prints a metrics table with segments, vias, total length, quality score
- `RoutingMetrics` from `calculate_metrics()` is the programmatic inspection surface

## Deviations

- The task plan called for creating `postprocess.rs` from scratch, but `paths_to_segments()` already existed inline in `orchestrator.rs` from T03. Extracted and improved it instead of writing from zero — same end result, cleaner migration.
- Fixed a pre-existing clippy warning in `cypcb-core/src/units.rs` (derivable `Default` impl) since it was blocking the clippy verification pipeline.

## Known Issues

- WASM compile (`wasm32-unknown-unknown`) fails due to `getrandom` crate — this is a transitive dependency issue, not related to cypcb-autoroute code. Deferred to T05.
- Upstream crates (cypcb-parser, cypcb-core) have pre-existing clippy warnings that block `cargo clippy` from completing, but cypcb-autoroute itself has zero warnings.

## Files Created/Modified

- `crates/cypcb-autoroute/src/postprocess.rs` — new module: path simplification, coordinate conversion, output types
- `crates/cypcb-autoroute/src/lib.rs` — added `pub mod postprocess`, switched route_board() to use postprocess::paths_to_output()
- `crates/cypcb-autoroute/src/orchestrator.rs` — removed old paths_to_segments() and make_segment() (replaced by postprocess module)
- `crates/cypcb-autoroute/tests/integration.rs` — enhanced with JLCPCB width/drill assertions, layer validation, quality bounds, apply_routes() compatibility test
- `crates/cypcb-core/src/units.rs` — fixed derivable Default clippy warning
