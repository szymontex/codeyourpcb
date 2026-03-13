# S02: 3D View Fix & Component Rendering

**Goal:** Fix the empty green board in 3D view and deliver visible component bodies, copper traces, pads, and vias — plus the GLB model loading pipeline that S06 will populate with real JLCPCB models.
**Demo:** Load `blink.cypcb`, toggle to 3D, see component bodies (dark gray ICs, tan passives) with refdes labels, pads as metallic shapes, and the PCB substrate — not an empty green board. Debug surface reports `componentCount > 0`, `meshCount > 1`.

## Must-Haves

- JS parser (`parseSource()`) sets `body_width_nm`, `body_height_nm` computed from pad bounding box, and `model_3d: null` on every ComponentInfo
- NaN guard in `buildComponents()` catches `undefined`, `NaN`, `0`, and negative values (use `!(x > 0)` pattern)
- `__renderer3d` debug surface extended with `componentCount`, `traceSegmentCount`, `padCount`, `viaCount`
- `loadComponentModel(url: string, refdes: string)` method on Renderer3D using Three.js GLTFLoader — loads GLB, positions at component location, replaces placeholder box
- GLTFLoader imported within lazy-loaded renderer3d module (not in main bundle)
- Proper cleanup of loaded GLTF scenes on dispose (geometry + material traversal)
- E2E tests verify: `componentCount > 0` and `meshCount > 1` after toggling 3D with blink.cypcb loaded
- Existing 3D toggle tests and FPS test continue to pass

## Proof Level

- This slice proves: contract (GLTFLoader/loadComponentModel boundary for S06) + operational (3D renders real geometry)
- Real runtime required: yes (WebGL rendering in browser)
- Human/UAT required: no (debug surface counts are sufficient — headless pixel comparison unreliable per existing decision)

## Verification

- `cd viewer && npx vitest run --reporter=verbose` — existing unit tests pass
- `cd viewer && npx playwright test e2e/three-d-view.spec.ts` — extended tests verify geometry counts
- `cd viewer && npx playwright test e2e/performance.spec.ts` — FPS ≥30 still passes
- Manual: load blink.cypcb → 3D toggle → component bodies visible with correct materials, refdes labels readable

## Observability / Diagnostics

- Runtime signals: `window.__renderer3d` exposes `componentCount`, `traceSegmentCount`, `padCount`, `viaCount` alongside existing `isActive`, `meshCount`, `drawCalls`, `fps` — all readable via `page.evaluate()` in E2E
- Failure visibility: `[3D] Warning: component ${refdes} has no body dimensions` console log on pad-bbox fallback; `[3D] GLB load failed for ${refdes}: ${error}` on model load failure
- Inspection: console `__renderer3d` object in dev tools for live geometry counts

## Integration Closure

- Upstream surfaces consumed: `BoardSnapshot` types (unchanged), `parseSource()` in wasm.ts (modified), existing Three.js infrastructure in renderer3d.ts
- New wiring introduced: `loadComponentModel(url, refdes)` public method on Renderer3D, enriched `__renderer3d` debug surface, GLTFLoader import
- Downstream consumers: S06 calls `loadComponentModel()` with JLCPCB GLB URLs; S07 E2E tests read enriched debug surface

## Tasks

- [x] **T01: Fix body dimensions pipeline, NaN guard, GLTFLoader integration** `est:2h`
  - Why: Root cause fix — JS parser doesn't set body dimensions, NaN propagates silently into invisible geometry. GLTFLoader is the boundary contract S06 needs.
  - Files: `viewer/src/wasm.ts`, `viewer/src/renderer3d.ts`
  - Do: (1) In `parseSource()`, compute `body_width_nm`/`body_height_nm` from pad bounding box when building ComponentInfo, set `model_3d: null`. (2) Fix NaN guard in `buildComponents()`: change `bodyW <= 0 || bodyH <= 0` to `!(bodyW > 0) || !(bodyH > 0)`. (3) Extend `updateDebugSurface()` with `componentCount`, `traceSegmentCount`, `padCount`, `viaCount` getters. (4) Import GLTFLoader from `three/examples/jsm/loaders/GLTFLoader.js`. Add `loadComponentModel(url, refdes)` method: loads GLB, finds component mesh by name, replaces placeholder box with loaded scene at same position/rotation/scale. (5) Extend `clearBoardGroup()` disposal traversal to handle GLTF scene graphs. (6) Track loaded model URLs to skip duplicate loads and clean up on dispose.
  - Verify: `cd viewer && npx vitest run` passes; load blink.cypcb → 3D → browser console shows component count > 0 in `__renderer3d`; no NaN warnings in console
  - Done when: Components visible in 3D, debug surface reports geometry counts, `loadComponentModel` method exists and handles load/error/dispose lifecycle

- [x] **T02: E2E tests for 3D geometry verification** `est:1h`
  - Why: Objective proof that the 3D view renders real geometry, not an empty board. Creates the regression gate for 3D rendering.
  - Files: `viewer/e2e/three-d-view.spec.ts`
  - Do: (1) Add test "3D view renders component geometry": load app → `__loadBoard(blinkSource)` → click 3D → verify `__renderer3d.componentCount > 0`, `__renderer3d.meshCount > 1`. (2) Add test "3D debug surface reports geometry counts": toggle 3D → verify all four new counters (`componentCount`, `traceSegmentCount`, `padCount`, `viaCount`) are numbers ≥ 0. (3) Add test "3D toggle preserves geometry on re-toggle": 3D on → check counts → 2D → 3D on again → counts match. (4) Ensure existing toggle and dispose tests still pass unchanged.
  - Verify: `cd viewer && npx playwright test e2e/three-d-view.spec.ts` all pass; `cd viewer && npx playwright test e2e/performance.spec.ts` still passes
  - Done when: All 3D E2E tests pass headless, componentCount > 0 verified for blink.cypcb, no regressions in existing test suite

## Files Likely Touched

- `viewer/src/wasm.ts` (fix parseSource — set body_width_nm, body_height_nm, model_3d)
- `viewer/src/renderer3d.ts` (fix NaN guard, extend debug surface, add GLTFLoader + loadComponentModel, extend dispose)
- `viewer/e2e/three-d-view.spec.ts` (extend with geometry verification tests)
