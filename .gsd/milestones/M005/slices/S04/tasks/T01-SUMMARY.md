---
id: T01
parent: S04
milestone: M005
provides:
  - VariantResultResponse with snapshot field in worker protocol
  - transformVariantResults() function mapping Rust VariantResult → TS VariantData
  - RawVariantResult type for Rust serde JSON shape
  - Unit tests for variant data transformation
key_files:
  - viewer/src/worker-protocol.ts
  - viewer/src/routing-worker.ts
  - viewer/src/variant-transform.ts
  - viewer/src/__tests__/variant-transform.test.ts
key_files_not_in_repo:
  - viewer/src/worker-protocol.ts - no commit in this clone ever added it (checked 2026-08-27)
  - viewer/src/routing-worker.ts - no commit in this clone ever added it (checked 2026-08-27)
  - viewer/src/variant-transform.ts - no commit in this clone ever added it (checked 2026-08-27)
  - viewer/src/__tests__/variant-transform.test.ts - no commit in this clone ever added it (checked 2026-08-27)
key_decisions:
  - Route segments grouped by net_id+layer into multi-segment route entries (not one-entry-per-segment)
patterns_established:
  - Rust serde JSON → TypeScript interface transformation with net_id→net_name map, Point→tuple, and position flattening
observability_surfaces:
  - "[Variants] Transformed N variants" log on successful transformation
  - "[Variants] Failed to parse variant result: <error>" log on JSON parse failure
  - Fallback net names ("net_<id>") visible in UI when net lookup misses
duration: 12m
verification_result: passed
completed_at: 2026-03-19T18:41:00Z
blocker_discovered: false
---

# T01: Add snapshot to variant-result protocol and build data transformation

**Added snapshot field to variant-result protocol and built Rust→TypeScript variant data transformation with 11 unit tests.**

## What Happened

Three changes were made:

1. **Protocol fix** — Added `snapshot: BoardSnapshot` to `VariantResultResponse` in `worker-protocol.ts`. Updated the `route-variants` handler in `routing-worker.ts` to call `engine.get_snapshot()` before `engine.free()` and include the snapshot in the response message.

2. **Data transformation layer** — Created `variant-transform.ts` with `transformVariantResults(rawJson, nets)` that maps the Rust-serialized format to TypeScript's `VariantData[]`. The function handles: `net_id` → `net_name` resolution via a lookup map (with `net_<id>` fallback), `Point {x,y}` → `[x,y]` tuple conversion for route segments, `position.x/y` → flat `x`/`y` for vias, route segment grouping by `net_id+layer`, and sorting by composite score ascending.

3. **Unit tests** — Created 11 tests covering: net name resolution, fallback names, Point→tuple conversion, via flattening, route grouping, score passthrough, name passthrough, composite sort order, invalid JSON handling (returns `[]` + error log), empty array, and empty variant (no routes/vias).

## Verification

- `npx tsc --noEmit` — zero errors
- `npx vitest run` — 138 tests passed (12 files), including 11 new variant-transform tests
- `npx vite build` — built successfully in 24.21s, worker bundles correctly

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `npx tsc --noEmit` | 0 | ✅ pass | 2.1s |
| 2 | `npx vitest run` | 0 | ✅ pass | 2.1s |
| 3 | `npx vite build` | 0 | ✅ pass | 25.0s |
| 4 | `npx playwright test e2e/variant-panel.spec.ts` | — | ⏭️ deferred to T04 | — |
| 5 | `npx playwright test e2e/autoroute-worker.spec.ts` | — | ⏭️ deferred to T04 | — |

## Diagnostics

- **Transform success:** Console shows `[Variants] Transformed N variants` after `transformVariantResults()` completes.
- **Transform failure:** Console shows `[Variants] Failed to parse variant result: <error>` on invalid JSON — function returns `[]` gracefully.
- **Fallback net names:** When a `net_id` from Rust has no match in `NetInfo[]`, the output uses `net_<id>` as the name — visible in variant panel UI rows.
- **Worker snapshot:** The `route-variants` handler now calls `engine.get_snapshot()` — the snapshot is included in the posted `variant-result` message for main-thread canvas updates (wired in T02).

## Deviations

None — implementation follows the task plan exactly.

## Known Issues

None.

## Files Created/Modified

- `viewer/src/worker-protocol.ts` — Added `snapshot: BoardSnapshot` field to `VariantResultResponse`
- `viewer/src/routing-worker.ts` — Updated `route-variants` handler to call `get_snapshot()` and include snapshot in response
- `viewer/src/variant-transform.ts` — **New** — Exports `transformVariantResults()` and `RawVariantResult` interface
- `viewer/src/__tests__/variant-transform.test.ts` — **New** — 11 unit tests for the transformation logic
- `.gsd/milestones/M005/slices/S04/tasks/T01-PLAN.md` — Added Observability Impact section (pre-flight fix)
