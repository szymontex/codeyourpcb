---
estimated_steps: 6
estimated_files: 6
---

# T01: Extend spatial index to include traces/vias and add trace-level DRC clearance

**Slice:** S03 — Renderer Upgrade & Manual Trace Editing
**Milestone:** M002

## Description

The spatial index currently only indexes component entities (via footprint bounds). Trace and via entities are invisible to spatial queries, which means DRC can't check trace-to-pad or trace-to-trace clearance, and the viewer can't hit-test traces for selection. This task extends `rebuild_spatial_index` to also index trace segments and vias, adds trace entity IDs to the snapshot types, and extends the clearance rule with trace-level checking including segment-to-segment distance math.

This is the foundation for everything else in S03 — without trace entities in the spatial index, neither DRC feedback nor trace selection can work.

## Steps

1. Add `id: u32` field to `TraceInfo` and `ViaInfo` in `crates/cypcb-render/src/snapshot.rs`. Update the TS mirror types in `viewer/src/types.ts` to add `id: number` to both `TraceInfo` and `ViaInfo`.

2. Update `collect_traces()` and `collect_vias()` in `crates/cypcb-render/src/lib.rs` to populate the `id` field from the entity's `Entity::index()`. Query with `(Entity, &Trace)` and `(Entity, &Via)` to get entity IDs alongside component data.

3. Add `rebuild_spatial_index_with_traces()` method to `BoardWorld` in `crates/cypcb-world/src/world.rs`. This method extends the existing `rebuild_spatial_index()` pattern: after indexing components, also iterate all `Trace` entities — for each trace segment, compute an AABB expanded by half the trace width on each side, create a `SpatialEntry` with the trace entity and the appropriate layer mask. Also index `Via` entities as circular AABBs (outer_diameter/2 on each side). Uses the `Layer::to_copper_mask()` method for layer mask conversion.

4. Call `rebuild_spatial_index_with_traces()` after `load_routes()` and after `populate_from_snapshot()` in `crates/cypcb-render/src/lib.rs`, passing the footprint library for component bounds.

5. Add segment-to-segment distance calculation utility in `crates/cypcb-drc/src/rules/clearance.rs`. The existing clearance rule uses AABB distance which is sufficient for component courtyards but is an overestimate for traces (trace AABBs are wider than the actual trace). Add a `segment_distance(seg1_start, seg1_end, seg2_start, seg2_end) -> i64` function that computes the exact minimum distance between two line segments (perpendicular distance + endpoint distances + projection). Use this for trace-to-trace clearance refinement after the AABB candidate filter.

6. Add unit tests: (a) `BoardWorld` test spawning traces, calling `rebuild_spatial_index_with_traces()`, and verifying spatial queries return trace entities. (b) DRC clearance test with a trace entity too close to a pad (component). (c) DRC clearance test with two trace entities too close to each other. (d) Segment-to-segment distance unit tests for parallel, perpendicular, and endpoint cases.

## Must-Haves

- [ ] `TraceInfo` and `ViaInfo` include `id` field in both Rust and TS
- [ ] Spatial index includes trace segment entries and via entries after rebuild
- [ ] DRC clearance rule detects trace-to-pad violations
- [ ] DRC clearance rule detects trace-to-trace violations
- [ ] Segment-to-segment distance function is correct for edge cases (parallel, perpendicular, endpoint)
- [ ] All existing tests still pass (no regressions)

## Verification

- `cargo test -p cypcb-world` — including new spatial index trace tests
- `cargo test -p cypcb-drc` — including new trace clearance tests
- `cargo test -p cypcb-render` — snapshot serialization tests pass with new `id` fields
- `cargo test` — full workspace, no regressions

## Observability Impact

- Signals added/changed: `SpatialIndex::len()` now includes trace/via entries (previously only components). DRC violation messages include trace entity references.
- How a future agent inspects this: `cargo test -p cypcb-drc -- clearance --nocapture` prints violation details. `SpatialIndex::len()` returns total indexed entries.
- Failure state exposed: DRC violations now surface trace-related clearance issues that were previously invisible.

## Inputs

- `crates/cypcb-world/src/spatial.rs` — existing R*-tree spatial index (component-only)
- `crates/cypcb-world/src/world.rs` — `rebuild_spatial_index()` method (component-only)
- `crates/cypcb-world/src/components/trace.rs` — `Trace`/`Via` components with `Layer`, `Nm` fields
- `crates/cypcb-drc/src/rules/clearance.rs` — existing clearance rule using AABB distance
- `crates/cypcb-render/src/snapshot.rs` — `TraceInfo`/`ViaInfo` snapshot types
- S02 T05 summary — explicitly notes "DRC trace-level clearance checking not yet supported"

## Expected Output

- `crates/cypcb-world/src/world.rs` — `rebuild_spatial_index_with_traces()` method added
- `crates/cypcb-drc/src/rules/clearance.rs` — trace-to-pad and trace-to-trace clearance checking, segment distance utility
- `crates/cypcb-render/src/snapshot.rs` — `id` field added to `TraceInfo` and `ViaInfo`
- `crates/cypcb-render/src/lib.rs` — `collect_traces()`/`collect_vias()` populate `id`; spatial index rebuilt with traces
- `viewer/src/types.ts` — `id` field added to `TraceInfo` and `ViaInfo` interfaces
