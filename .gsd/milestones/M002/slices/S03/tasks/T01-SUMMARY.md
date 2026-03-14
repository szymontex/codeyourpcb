---
id: T01
parent: S03
milestone: M002
provides:
  - Trace and via entities indexed in spatial R*-tree
  - Trace-aware DRC clearance checking (trace-to-pad, trace-to-trace)
  - Segment-to-segment distance function for precise trace clearance
  - Entity IDs on TraceInfo/ViaInfo for selection/hit-testing
key_files:
  - crates/cypcb-world/src/world.rs
  - crates/cypcb-drc/src/rules/clearance.rs
  - crates/cypcb-render/src/snapshot.rs
  - crates/cypcb-render/src/lib.rs
  - viewer/src/types.ts
key_decisions:
  - Trace segments get per-segment AABBs expanded by half-width; all segments of a trace share the same Entity in the spatial index
  - Via layer mask is the OR of start_layer and end_layer copper masks (not all-layers)
  - Segment-to-segment distance uses i128 intermediates and f64 for closest-point computation, clamped parametric approach
  - Trace-to-AABB distance checks segment endpoints and edges against AABB sides, with early-exit for overlap
patterns_established:
  - rebuild_spatial_index_with_traces() is the canonical full rebuild; sync.rs and PcbEngine both use it
  - TraceData struct pre-collects trace geometry for DRC to avoid repeated ECS queries during clearance checking
observability_surfaces:
  - SpatialIndex::len() now returns total entries including traces/vias (was components-only)
  - DRC violations include trace entity references in clearance messages
  - cargo test -p cypcb-drc -- clearance --nocapture prints trace clearance violation details
duration: 1 session
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Extended spatial index and DRC to cover traces/vias

**Added trace/via entities to the spatial index and implemented trace-aware DRC clearance checking with precise segment-to-segment distance math.**

## What Happened

1. Added `id: u32` field to `TraceInfo` and `ViaInfo` in Rust snapshot types and `id: number` in the TS mirror types. Updated all constructors (production and test) to populate the field.

2. Updated `collect_traces()` and `collect_vias()` in PcbEngine to query `(Entity, &Trace)` / `(Entity, &Via)` and populate the `id` field from `Entity::index()`.

3. Added `rebuild_spatial_index_with_traces()` to `BoardWorld`. It extends the component-only pattern: indexes components (same as before), then indexes each trace segment as an AABB expanded by half-width with the trace's copper layer mask, then indexes each via as a circular AABB with OR'd start/end layer masks.

4. Updated all call sites to use `rebuild_spatial_index_with_traces()`: `populate_from_snapshot()`, `load_routes()`, and `sync_ast_to_world()`. Created `rebuild_spatial_index_full()` helper on PcbEngine to avoid duplicating the footprint_lib closure.

5. Implemented `segment_distance()` function using the parametric closest-point algorithm for two line segments. Handles degenerate cases (point-to-point, point-to-segment), parallel segments, and general configurations. Uses i128 intermediates for overflow safety. Added `trace_to_trace_distance()` and `trace_to_aabb_distance()` helpers.

6. Extended the clearance rule's `check()` method to pre-collect trace geometry into a `TraceData` map. When either entity in a clearance pair is a trace, the rule uses refined segment distance instead of raw AABB distance: trace-to-trace subtracts both half-widths from segment distance, trace-to-pad subtracts one half-width from segment-to-AABB distance.

7. Added 11 new tests across cypcb-world (4 spatial index tests with traces/vias) and cypcb-drc (7 segment distance + 4 trace DRC tests).

## Verification

- `cargo test -p cypcb-world -- spatial` — 14 passed (4 new: traces, vias, combined, layer mask)
- `cargo test -p cypcb-drc -- clearance` — 36 passed (11 new: 7 segment distance + 4 trace DRC)
- `cargo test -p cypcb-render -- trace` — 2 passed (snapshot serialization with new id fields)
- `cargo test -p cypcb-world -p cypcb-drc -p cypcb-render -p cypcb-core -p cypcb-parser` — 133 passed, 1 pre-existing failure (test_sync_named_pin, unrelated)

## Diagnostics

- `SpatialIndex::len()` returns total indexed entries (components + trace segments + vias)
- `cargo test -p cypcb-drc -- clearance --nocapture` shows trace-related violation details
- `segment_distance()` is a public function, unit-testable in isolation

## Deviations

- Also updated `sync_ast_to_world()` in `sync.rs` to use `rebuild_spatial_index_with_traces()` — this wasn't in the task plan but is the third call site that builds the spatial index. Without it, traces defined in DSL source would be invisible to DRC.

## Known Issues

- Pre-existing: `sync::tests::test_sync_named_pin` fails (not related to this task, was already failing before changes)
- `point_to_segment_distance()` helper is defined but only used internally via `segment_distance(p, p, s1, s2)` — may want to inline or expose it later

## Files Created/Modified

- `crates/cypcb-world/src/world.rs` — Added `rebuild_spatial_index_with_traces()` method + 4 spatial index tests
- `crates/cypcb-world/src/sync.rs` — Changed `rebuild_spatial_index` → `rebuild_spatial_index_with_traces`
- `crates/cypcb-drc/src/rules/clearance.rs` — Added segment distance math, TraceData, trace-aware clearance checking, 11 new tests
- `crates/cypcb-render/src/snapshot.rs` — Added `id: u32` to TraceInfo and ViaInfo, updated test constructors
- `crates/cypcb-render/src/lib.rs` — Updated collect_traces/collect_vias to include entity IDs, added `rebuild_spatial_index_full()`, call it after load_routes and populate_from_snapshot
- `viewer/src/types.ts` — Added `id: number` to TraceInfo and ViaInfo interfaces
