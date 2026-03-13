---
id: T03
parent: S03
milestone: M002
provides:
  - PcbEngine mutation API: add_trace, remove_trace, get_trace_at_point, run_drc_incremental, trace_count
  - WASM bridge for all mutation methods via wasm_bindgen
  - MockPcbEngine parity with same mutation API for dev mode
  - Point-to-segment distance utility for trace hit-testing
  - get_violations_json for WASM-friendly DRC result access
key_files:
  - crates/cypcb-render/src/lib.rs
  - viewer/src/wasm.ts
  - viewer/src/types.ts
key_decisions:
  - Segments passed as flat i64 array [x1,y1,x2,y2,...] instead of JSON objects — avoids serde overhead on the hot path; WASM variant add_trace_json accepts JSON string for JS interop
  - Entity lookup by index uses linear scan of trace entities rather than Entity::from_raw — generation mismatch after despawn would cause silent failures with from_raw
  - MockPcbEngine uses simple endpoint-based segment-to-segment distance for DRC — sufficient for preview feedback; real DRC is in Rust
  - WASM adapter invalidates cachedSnapshot on mutation — forces fresh snapshot from Rust engine on next get_snapshot()
patterns_established:
  - Mutation methods return u32::MAX (0xFFFFFFFF) as error sentinel for entity IDs — matches JS convention where the value is checked as `!== 0xFFFFFFFF`
  - point_to_segment_distance() is a module-level utility — same algorithm used in both Rust (PcbEngine) and TypeScript (MockPcbEngine)
  - rebuild_spatial_index_full() called after every mutation — ensures spatial index stays consistent; incremental updates deferred to performance optimization
observability_surfaces:
  - engine.trace_count() — number of trace entities (both WASM and Mock)
  - engine.run_drc_incremental() — returns violation count after full DRC recheck
  - MockPcbEngine logs [MockEngine] add_trace/remove_trace/run_drc_incremental to console
  - engine.get_violations_json() — full DRC violations as JSON string
duration: 35min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T03: WASM bridge mutation API and MockPcbEngine parity

**Added trace add/remove/query/DRC mutation API to PcbEngine (Rust+WASM) and MockPcbEngine (JS), with full test coverage for the add→query→remove cycle.**

## What Happened

Added five mutation methods to the Rust `PcbEngine`: `add_trace()`, `remove_trace()`, `get_trace_at_point()`, `run_drc_incremental()`, and `trace_count()`. Each method is exposed via `#[wasm_bindgen]` with a WASM-specific `add_trace_json()` variant that accepts a JSON string of coordinates (since WASM can't pass Rust slices directly from JS).

The TypeScript `PcbEngine` interface was extended with matching method signatures. The `WasmPcbEngineAdapter` proxies calls to the raw WASM engine, converting JS numbers to BigInt where needed and invalidating the cached snapshot on mutations.

`MockPcbEngine` implements the same API using pure JS trace storage. Its `add_trace` validates inputs and assigns monotonic entity IDs starting at 1000. Its `run_drc_incremental` implements a basic clearance check between traces on the same layer using segment-to-segment distance.

Two geometry utility functions were added: `point_to_segment_distance` (in both Rust and TS) for trace hit-testing, and `segmentToSegmentDistance` (TS only) for mock DRC.

## Verification

- `cargo test -p cypcb-render -- trace` — 14 tests pass (add, remove, query, multi-segment, bad inputs, add-remove-add cycle, point distance)
- `cargo test -p cypcb-render` — all 31 unit tests + 1 doc-test pass
- `cargo test -p cypcb-world -- spatial` — 14 tests + 5 doc-tests pass
- `cargo test -p cypcb-drc -- clearance` — 36 tests + 2 doc-tests pass
- `npx tsc --noEmit` — TypeScript compiles clean with no errors

## Diagnostics

- `cargo test -p cypcb-render -- trace --nocapture` — shows trace mutation test details
- `engine.trace_count()` — programmatic inspection of trace entity count
- `engine.run_drc_incremental()` — returns violation count
- MockPcbEngine console logs prefixed with `[MockEngine]` for add/remove/DRC operations

## Deviations

- Added `get_violations_json()` method (not in original plan) — needed for WASM to return DRC violations as a serializable string rather than requiring full snapshot rebuild.

## Known Issues

- `point_to_segment_distance` function shows dead_code warning in native test builds — it's only called from methods in the wasm_bindgen impl block. Harmless.
- `run_drc_incremental()` currently runs a full DRC check, not incremental — plan explicitly defers incremental optimization.

## Files Created/Modified

- `crates/cypcb-render/src/lib.rs` — Added trace mutation API (add_trace, remove_trace, get_trace_at_point, run_drc_incremental, trace_count, get_violations_json), find_trace_entity helper, point_to_segment_distance utility, 14 new tests
- `viewer/src/wasm.ts` — Extended PcbEngine interface with mutation methods; implemented same API in WasmPcbEngineAdapter and MockPcbEngine; added pointToSegmentDistance and segmentToSegmentDistance utilities
