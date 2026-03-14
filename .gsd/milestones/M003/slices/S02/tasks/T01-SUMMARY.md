---
id: T01
parent: S02
milestone: M003
provides:
  - body_width_nm / body_height_nm computed from pad bounding box in parseSource()
  - model_3d: null set on every ComponentInfo
  - NaN-safe guard in buildComponents() using !(x > 0) pattern
  - componentCount, traceSegmentCount, padCount, viaCount on __renderer3d debug surface
  - loadComponentModel(url, refdes) method with load/replace/error/dispose lifecycle
  - GLTFLoader imported in lazy-loaded renderer3d.ts module
  - GLTF scene disposal in clearBoardGroup()
key_files:
  - viewer/src/wasm.ts
  - viewer/src/renderer3d.ts
key_decisions:
  - "Body dimensions computed from pad bounding box at parse time — avoids cross-boundary Rust change"
  - "NaN guard uses !(x > 0) pattern — catches NaN, undefined, 0, and negative"
  - "GLTFLoader imported inside lazy-loaded renderer3d.ts — keeps Three.js out of main bundle"
  - "loadComponentModel replaces placeholder box by name convention (component-{refdes})"
patterns_established:
  - "Geometry count tracking via private instance fields, exposed through debug surface getters"
  - "GLTF model lifecycle: load → find placeholder → copy transform → remove placeholder → add model → track in Map → dispose on clear"
observability_surfaces:
  - "window.__renderer3d.componentCount — number of component body meshes in 3D scene"
  - "window.__renderer3d.traceSegmentCount — total trace segments rendered"
  - "window.__renderer3d.padCount — total pads rendered"
  - "window.__renderer3d.viaCount — total vias rendered"
  - "[3D] Warning: component ${refdes} has no body dimensions — console log on pad-bbox fallback"
  - "[3D] GLB load failed for ${refdes}: ${error} — console error on model load failure"
duration: 30m
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Fix body dimensions pipeline, NaN guard, GLTFLoader integration

**Fixed root cause of empty 3D board (undefined body dimensions → NaN → invisible meshes), added NaN-safe guard, enriched debug surface with geometry counts, and integrated GLTFLoader for S06 model loading.**

## What Happened

The JS parser in `parseSource()` was building `ComponentInfo` without setting `body_width_nm`, `body_height_nm`, or `model_3d`. These `undefined` values propagated through `comp.body_width_nm * NM_TO_MM` producing `NaN`, which the renderer's guard `bodyW <= 0` failed to catch (since `NaN <= 0 === false` in JS). `BoxGeometry(NaN, NaN, ...)` created invisible zero-size meshes.

Fix: compute body dimensions from the pad bounding box at parse time (min/max of pad x±width/2, y±height/2). This runs once per component during parse, not per render. The renderer fallback still exists for edge cases but now logs a warning.

The NaN guard was changed to `!(bodyW > 0) || !(bodyH > 0)` which correctly rejects NaN, undefined, 0, and negative values.

Extended the `__renderer3d` debug surface with `componentCount`, `traceSegmentCount`, `padCount`, `viaCount` — all tracked as instance fields during build methods and exposed as getters.

Added `loadComponentModel(url, refdes)` using Three.js GLTFLoader. It finds the placeholder box mesh by name `component-${refdes}`, clones its position/rotation, removes it, loads the GLB, and adds the GLTF scene at the same transform. Loaded models are tracked in a `Map<string, THREE.Group>` for cleanup. `clearBoardGroup()` now traverses GLTF scene graphs to dispose nested geometries, materials, and textures.

## Verification

- `cd viewer && npx vitest run --reporter=verbose` — **63 tests passed**, zero failures
- `cd viewer && npx playwright test e2e/three-d-view.spec.ts` — **3 tests passed** (3D toggle, keyboard shortcut, dispose)
- `cd viewer && npx playwright test e2e/performance.spec.ts` — **2 tests passed** (load time, FPS ≥30 at 60fps)
- `cd viewer && npx playwright test` — **49 tests passed**, zero failures (full E2E suite)
- Inline Playwright test confirmed: `__renderer3d.componentCount === 9` with blink.cypcb loaded in 3D, `meshCount === 12`, `padCount === 24`

## Diagnostics

- `window.__renderer3d` in browser console shows all geometry counts live
- `[3D] Warning: component ${refdes} has no body dimensions` logged when pad-bbox fallback is used in renderer
- `[3D] GLB load failed for ${refdes}: ${error}` logged when model loading fails
- `[3D] GLB loaded for ${refdes}: ${url}` logged on successful model load
- Geometry counts reset to 0 on `clearBoardGroup()` — zero counts after dispose confirms cleanup

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `viewer/src/wasm.ts` — parseSource() now computes body_width_nm/body_height_nm from pad bbox, sets model_3d: null
- `viewer/src/renderer3d.ts` — NaN-safe guard, GLTFLoader import, loadComponentModel method, enriched debug surface, extended GLTF disposal
