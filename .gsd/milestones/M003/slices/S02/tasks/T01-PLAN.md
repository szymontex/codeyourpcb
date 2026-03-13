# T01: Fix body dimensions pipeline, NaN guard, GLTFLoader integration

## Description

Fix the root cause of the empty 3D board: the JS parser in `parseSource()` doesn't set `body_width_nm` or `body_height_nm` on ComponentInfo, producing `undefined` values that propagate as NaN into Three.js BoxGeometry and create invisible meshes. Then harden the renderer's NaN guard, extend the debug surface for E2E testability, and add GLTFLoader integration as the boundary contract for S06.

## Steps

1. **Fix JS parser body dimensions** — In `viewer/src/wasm.ts` `parseSource()`, after building the component's pad array via `getFootprintPads()`, compute body dimensions from the pad bounding box (min/max of pad x±width/2, y±height/2). Set `body_width_nm`, `body_height_nm` on every `ComponentInfo`. Also set `model_3d: null` (field exists in type but never populated).

2. **Fix NaN guard in renderer** — In `viewer/src/renderer3d.ts` `buildComponents()`, change `if (bodyW <= 0 || bodyH <= 0)` to `if (!(bodyW > 0) || !(bodyH > 0))`. This catches NaN, undefined, 0, and negative — the original condition passes NaN through because `NaN <= 0 === false`.

3. **Extend debug surface** — In `updateDebugSurface()`, add getters: `componentCount` (count of `component-*` named meshes in scene), `traceSegmentCount` (count from buildTraces), `padCount` (from buildPads), `viaCount` (from buildVias). Store counts as instance fields during build methods.

4. **Add GLTFLoader integration** — Import `GLTFLoader` from `three/examples/jsm/loaders/GLTFLoader.js` (already in node_modules via three@0.183.2). Add `loadComponentModel(url: string, refdes: string): void` method on Renderer3D that: loads GLB via GLTFLoader, finds the placeholder box mesh by name `component-${refdes}`, copies its position/rotation, removes it, adds the loaded GLTF scene at the same transform. Log errors to console on load failure. Track loaded URLs in a `Map<string, THREE.Group>` for cleanup.

5. **Extend dispose cleanup** — In `clearBoardGroup()`, traverse loaded GLTF scenes and dispose their geometries and materials (GLTF scene graphs have nested children with separate materials). Clear the loaded models map.

6. **Verify manually** — Start dev server, load blink.cypcb, toggle 3D. Confirm: component bodies visible (not empty green board), `__renderer3d.componentCount > 0` in console, no NaN warnings.

## Must-Haves

- `body_width_nm` and `body_height_nm` set from pad bbox for every component in `parseSource()`
- `model_3d: null` explicitly set on ComponentInfo
- NaN-safe guard using `!(x > 0)` pattern
- `componentCount`, `traceSegmentCount`, `padCount`, `viaCount` on `__renderer3d`
- `loadComponentModel(url, refdes)` method exists and handles: load, position, replace placeholder, error logging, dispose
- GLTFLoader imported within renderer3d.ts (lazy-loaded module), not in main bundle

## Verification

- `cd viewer && npx vitest run --reporter=verbose` — no regressions
- Load blink.cypcb in browser → 3D toggle → component bodies visible
- `window.__renderer3d.componentCount` returns number > 0 in browser console
- No `NaN` or `undefined * number` errors in console

## Inputs

- Research findings confirming root cause (NaN from undefined body_width_nm)
- Existing `getFootprintPads()` returns valid pad arrays with x/y/width/height
- Three.js 0.183.2 in node_modules with GLTFLoader at standard path

## Expected Output

- `viewer/src/wasm.ts` — parseSource sets body dimensions and model_3d on every component
- `viewer/src/renderer3d.ts` — NaN guard fixed, debug surface enriched, GLTFLoader integrated, dispose extended
- Components render as visible 3D boxes with correct materials and positions
- Debug surface reports accurate geometry counts
