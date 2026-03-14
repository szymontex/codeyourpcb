---
id: S02
parent: M003
milestone: M003
provides:
  - body_width_nm / body_height_nm computed from pad bounding box in parseSource()
  - model_3d: null set on every ComponentInfo
  - NaN-safe guard in buildComponents() using !(x > 0) pattern
  - componentCount, traceSegmentCount, padCount, viaCount on __renderer3d debug surface
  - loadComponentModel(url, refdes) method with load/replace/error/dispose lifecycle
  - GLTFLoader imported in lazy-loaded renderer3d.ts module
  - GLTF scene disposal in clearBoardGroup()
  - E2E regression tests for 3D geometry counts (componentCount > 0, meshCount > 1)
  - Re-toggle consistency test proving clearBoardGroup + rebuild is deterministic
requires:
  - slice: none
    provides: standalone — consumes existing BoardSnapshot types and Three.js infrastructure
affects:
  - S06 (calls loadComponentModel with JLCPCB GLB URLs)
  - S07 (E2E tests read enriched __renderer3d debug surface)
key_files:
  - viewer/src/wasm.ts
  - viewer/src/renderer3d.ts
  - viewer/e2e/three-d-view.spec.ts
key_decisions:
  - "Body dimensions computed from pad bounding box at parse time — avoids cross-boundary Rust change"
  - "NaN guard uses !(x > 0) pattern — catches NaN, undefined, 0, and negative"
  - "GLTFLoader imported inside lazy-loaded renderer3d.ts — keeps Three.js out of main bundle"
  - "loadComponentModel replaces placeholder box by name convention (component-{refdes})"
  - "Geometry assertions use debug surface counters, not pixel comparison — deterministic and fast"
patterns_established:
  - "Geometry count tracking via private instance fields, exposed through debug surface getters"
  - "GLTF model lifecycle: load → find placeholder → copy transform → remove placeholder → add model → track in Map → dispose on clear"
  - "activate3D() / getGeometryCounts() shared test helpers for 3D E2E tests"
observability_surfaces:
  - "window.__renderer3d.componentCount — number of component body meshes in 3D scene"
  - "window.__renderer3d.traceSegmentCount — total trace segments rendered"
  - "window.__renderer3d.padCount — total pads rendered"
  - "window.__renderer3d.viaCount — total vias rendered"
  - "[3D] Warning: component ${refdes} has no body dimensions — console log on fallback"
  - "[3D] GLB load failed for ${refdes}: ${error} — console error on model load failure"
drill_down_paths:
  - .gsd/milestones/M003/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M003/slices/S02/tasks/T02-SUMMARY.md
duration: 40m
verification_result: passed
completed_at: 2026-03-13
---

# S02: 3D View Fix & Component Rendering

**Fixed empty green board root cause (undefined body dimensions → NaN → invisible meshes), added GLTFLoader pipeline for S06, and locked it down with E2E geometry count tests.**

## What Happened

The 3D view was rendering an empty green board because `parseSource()` never set `body_width_nm` or `body_height_nm` on ComponentInfo objects. These `undefined` values propagated through `comp.body_width_nm * NM_TO_MM` producing `NaN`, and the guard `bodyW <= 0` failed silently (NaN <= 0 is false in JS). Result: `BoxGeometry(NaN, NaN, ...)` created zero-size invisible meshes.

**T01** fixed the pipeline end-to-end: (1) `parseSource()` now computes body dimensions from the pad bounding box at parse time. (2) The NaN guard was changed to `!(bodyW > 0)` which correctly rejects NaN, undefined, 0, and negative. (3) The `__renderer3d` debug surface was extended with `componentCount`, `traceSegmentCount`, `padCount`, `viaCount`. (4) `loadComponentModel(url, refdes)` was added using Three.js GLTFLoader — it finds the placeholder box by name, copies the transform, swaps in the loaded GLTF scene, and tracks models for disposal. This is the contract boundary S06 will use to load JLCPCB GLB models.

**T02** added 3 E2E tests proving the fix works: component geometry renders (componentCount > 0, meshCount > 1), all debug counters are valid numbers, and re-toggling 3D reconstructs identical geometry. Shared helpers (loadBlink, activate3D, getGeometryCounts) keep tests clean.

## Verification

- `npx vitest run` — 63 unit tests passed
- `npx playwright test e2e/three-d-view.spec.ts` — 6 tests passed (3 existing + 3 new geometry tests)
- `npx playwright test e2e/performance.spec.ts` — 2 tests passed (FPS ≥30 at 60fps)
- `npx playwright test` — 52 E2E tests passed, zero failures
- blink.cypcb in 3D: componentCount=9, meshCount=12, padCount=24

## Requirements Advanced

- None advanced — this slice fixed an existing broken capability rather than advancing new requirements.

## Requirements Validated

- None newly validated — 3D board preview was already marked validated from v2.0 (the framework existed, it just rendered empty).

## New Requirements Surfaced

- None.

## Requirements Invalidated or Re-scoped

- None.

## Deviations

None.

## Known Limitations

- Component bodies are procedural boxes — real 3D models require S06 to call `loadComponentModel()` with JLCPCB GLB URLs
- Traces render as flat quads, not realistic copper ribbons (acceptable for beta)
- `model_3d` is always `null` until S06 populates it from JLCPCB API

## Follow-ups

- S06 must call `loadComponentModel(url, refdes)` to replace placeholder boxes with real GLB models
- S07 should extend E2E coverage to verify loaded GLB models appear correctly

## Files Created/Modified

- `viewer/src/wasm.ts` — parseSource() computes body_width_nm/body_height_nm from pad bbox, sets model_3d: null
- `viewer/src/renderer3d.ts` — NaN-safe guard, GLTFLoader import, loadComponentModel method, enriched debug surface, extended GLTF disposal
- `viewer/e2e/three-d-view.spec.ts` — 3 geometry verification tests + shared helpers

## Forward Intelligence

### What the next slice should know
- `loadComponentModel(url, refdes)` is the entry point for S06 — pass a GLB URL and a refdes string, it handles everything (find placeholder, swap, track, dispose).
- The `__renderer3d` debug surface is the canonical way to verify 3D state in E2E tests. Don't try pixel comparison.

### What's fragile
- Body dimensions from pad bounding box are an approximation — components with odd pad layouts (e.g., QFN exposed pads extending beyond signal pads) may get oversized boxes. Real GLB models from S06 will fix this.
- The `component-${refdes}` naming convention is the linchpin for model replacement — if component mesh naming changes, `loadComponentModel` will fail silently (logs warning but doesn't crash).

### Authoritative diagnostics
- `window.__renderer3d` in browser console — shows live geometry counts. If componentCount is 0 after loading a board in 3D, the parse pipeline broke.
- Console warnings `[3D] Warning: component ${refdes} has no body dimensions` indicate the pad-bbox fallback path is being used.

### What assumptions changed
- Original assumption: 3D empty board might be a coordinate transform or rendering bug. Actual cause: undefined body dimensions in JS parser → NaN propagation past a broken guard. Pure data pipeline issue.
