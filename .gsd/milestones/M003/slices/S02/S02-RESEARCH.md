# S02: 3D View Fix & Component Rendering — Research

**Date:** 2026-03-13

## Summary

The "empty green board" in 3D has a confirmed root cause: **the JavaScript parser doesn't set `body_width_nm` or `body_height_nm` on ComponentInfo**, leaving them as `undefined`. The 3D renderer's fallback check (`if (bodyW <= 0 || bodyH <= 0)`) fails silently because `NaN <= 0` evaluates to `false` in JavaScript, so `BoxGeometry(NaN, NaN, compHeight)` produces invisible geometry. This affects both MockPcbEngine and WasmPcbEngineAdapter (which caches JS-parsed snapshots). Pads and traces DO render correctly when present — the "empty" appearance is because: (a) component bodies are invisible due to the NaN bug, and (b) without loaded routes, there are no traces or vias in the snapshot to render.

The fix is straightforward for the data pipeline. The larger slice scope — rendering traces as ribbons, pads as metallic shapes, vias as cylinders, and component bodies with correct dimensions — is already mostly implemented in `renderer3d.ts`. The missing pieces are: (1) the NaN body dimensions bug, (2) the JS parser not setting `body_width_nm`/`body_height_nm`/`model_3d`, (3) no GLB model loading pipeline, (4) no diagnostic surface for E2E verification of 3D geometry, and (5) the `__renderer3d` debug surface only exposes `isActive`, `meshCount`, `drawCalls`, `fps` — not component/trace/pad counts.

The boundary map requires S02 to produce a `loadComponentModel(url: string)` method on Renderer3D and confirmed GLTFLoader integration. Three.js 0.183.2 ships GLTFLoader at `three/examples/jsm/loaders/GLTFLoader.js`. The `model_3d` field on ComponentInfo is already typed as `string | null` — it just needs populating (which S06 will do for JLCPCB parts).

## Recommendation

Fix the data pipeline bugs first (JS parser body dimensions, NaN guard), then enhance the 3D debug surface for E2E testability, add GLTFLoader integration with `loadComponentModel()`, and write E2E tests verifying visible 3D geometry. The renderer3d.ts geometry builders (buildTraces, buildPads, buildVias, buildComponents) are already correct in structure — they just need the data fix and a few hardening improvements.

**Approach:**
1. Fix JS parser `parseSource()` to set `body_width_nm`, `body_height_nm`, `model_3d` on ComponentInfo — compute body dims from pad bounding box at parse time
2. Fix 3D renderer's NaN guard: change `bodyW <= 0 || bodyH <= 0` to `!(bodyW > 0) || !(bodyH > 0)` (catches NaN, undefined, 0, negative)
3. Extend `__renderer3d` debug surface with `componentCount`, `traceSegmentCount`, `padCount`, `viaCount` for E2E verification
4. Add `loadComponentModel(url: string, refdes: string)` method using Three.js GLTFLoader — loads a GLB and positions it at the component's location, replacing the placeholder box
5. Write E2E tests: load blink.cypcb → toggle 3D → verify meshCount > 1, componentCount > 0; load routed board → verify traceSegmentCount > 0
6. Verify the entire flow end-to-end in browser

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| GLB/GLTF model loading | `three/examples/jsm/loaders/GLTFLoader.js` (ships with three.js 0.183.2) | Industry standard, handles materials, hierarchy, animations; already in node_modules |
| 3D geometry primitives | Three.js BoxGeometry, CylinderGeometry, BufferGeometry | Already used throughout renderer3d.ts; no reason to change |
| Instanced rendering | THREE.InstancedMesh | Already used for vias; handles per-instance transforms efficiently |

## Existing Code and Patterns

- `viewer/src/renderer3d.ts` (872 lines) — Complete 3D renderer with scene setup, copper geometry builders, via instancing, component body rendering, refdes labels as sprites, layer visibility, FPS tracking, `__renderer3d` debug surface. All geometry code is structurally correct. **Fix:** NaN body dimension guard. **Extend:** GLTFLoader integration, enriched debug surface.
- `viewer/src/wasm.ts` `parseSource()` (lines ~250-320) — JS parser creates ComponentInfo with `Partial<ComponentInfo>` then casts. Missing `body_width_nm`, `body_height_nm`, `model_3d`. **Fix:** Compute body dimensions from pad bounding box at parse time, set `model_3d: null`.
- `viewer/src/wasm.ts` `getFootprintPads()` — Returns standard pad arrays for known footprints (0402, 0603, 0805, 1206, PIN-HDR-1x2, SOIC-8, DIP-8). These have valid x/y/width/height — body dims can be derived from them.
- `viewer/src/wasm.ts` `WasmPcbEngineAdapter.get_snapshot()` — Returns JS-cached snapshot (with JS-parsed body dims). After trace mutation, falls through to WASM engine's snapshot (Rust-computed, has proper body dims). The inconsistency between JS-parsed and WASM snapshots is a latent issue.
- `viewer/src/main.ts` (line 451) — 3D toggle calls `renderer3d.updateBoard(snapshot, layers)` — passes whatever snapshot is current. If the JS parser bug is fixed, this works.
- `viewer/e2e/three-d-view.spec.ts` — Existing tests check toggle activation and dispose. Need extension for geometry verification.
- `viewer/e2e/performance.spec.ts` — FPS test (≥30fps headless). Keep passing.
- S01's `window.__renderDiag` pattern — Diagnostic-driven E2E for 2D renderer. Should replicate for 3D with `__renderer3d`.

## Constraints

- **Canvas vs WebGL context exclusivity** — When 3D is active, the 2D canvas is hidden (`display: none`). Toggle back disposes the entire WebGL context. No persistent WebGL when not in use (existing decision).
- **Headless WebGL limitations** — Headless Chromium WebGL rendering varies; E2E tests should assert debug surface counts (meshCount, componentCount) not pixel comparisons (existing decision).
- **No WASM change required** — Body dimensions fix is JS-side only (parser + renderer guard). The Rust `build_snapshot()` already computes body dimensions correctly from the footprint library.
- **Three.js lazy-loaded** — `renderer3d.ts` is loaded via dynamic `import()`. GLTFLoader must be imported within the lazy-loaded module, not in the main bundle (existing decision).
- **GLB model URLs not yet available** — S06 (JLCPCB integration) will populate `model_3d` field. S02 should build the loading mechanism and verify it works with a test model, but production model URLs come later.
- **Snapshot data determines 3D content** — Without loaded routes (.ses/.routes), snapshot has no traces or vias. Component rendering is the primary visible improvement from S02. Trace/via rendering is already implemented and just needs data to render.

## Common Pitfalls

- **NaN propagation in JavaScript** — `undefined * number = NaN`, and `NaN` fails all comparison operators silently. Use `!(x > 0)` or `Number.isFinite(x) && x > 0` instead of `x <= 0` for numeric guards. This is the root cause of the current bug.
- **Partial<T> cast to T** — TypeScript's `Partial<ComponentInfo>` allows missing fields, but casting `as ComponentInfo` doesn't runtime-validate. Any field not explicitly set is `undefined`, not the type's default. The JS parser must explicitly set all required fields.
- **InstancedMesh matrix order** — `makeScale()` resets the entire matrix, then `setPosition()` only overwrites the translation column. This is correct but non-obvious. If scale and rotation are both needed, use `compose(position, quaternion, scale)` instead.
- **WebGL context loss** — If the user rapidly toggles 2D/3D, the dispose/init cycle must be complete before the next init. The current `renderer3d.dispose()` + `renderer3d = null` pattern handles this, but loading GLB models adds async state that needs cleanup on dispose.
- **GLTFLoader memory leaks** — Loaded GLTF scenes must have their geometries and materials disposed when removed. The existing `clearBoardGroup()` traverse handles Mesh and Sprite disposal but needs extension for GLTF scene graphs.

## Open Risks

- **GLB test model availability** — Need a sample GLB component model for E2E testing. Options: generate a simple box GLB as a test fixture, or use a public domain component model. Low risk — a minimal test model can be generated with Three.js and exported.
- **Performance with many GLB models** — A board with 50+ components each loading separate GLB files could cause HTTP waterfall. Mitigation: model loading should be async and incremental, not blocking the initial 3D render. Component boxes render immediately; GLB models replace them as they load. This is a S06 concern but the S02 API should account for it.
- **WasmPcbEngineAdapter snapshot inconsistency** — The JS-cached snapshot and the WASM engine's snapshot have different body dimension values (JS: computed from pads, WASM: from footprint library bounds). After any trace mutation that invalidates the cache, the snapshot switches to WASM-sourced. The 2D renderer works either way because it reads body dims from the same snapshot. But there could be subtle differences in body dimensions between the two paths. Low risk for S02 — document and defer full reconciliation.

## Requirements Supported

This slice directly supports:
- **3D view renders traces, pads, vias, and component bodies** (M003 success criteria)
- Indirectly advances: LIB-03 (User can associate 3D STEP models with components) — builds the GLB loading infrastructure that S06 will populate with real models

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Three.js | `cloudai-x/threejs-skills@threejs-fundamentals` (2K installs) | available — covers scene setup, geometry, materials |
| Three.js | `cloudai-x/threejs-skills@threejs-geometry` (1.6K installs) | available — relevant to BufferGeometry and instanced mesh |
| WebGL | `martinholovsky/claude-skills-generator@webgl` (159 installs) | available — low install count, probably not needed |

Three.js skills could be useful if geometry or material complexity increases, but the current renderer3d.ts is already well-structured. Not critical for S02 scope.

## Sources

- Root cause confirmed by tracing `undefined * 1e-6 = NaN` through `buildComponents()` and verifying `NaN <= 0 === false` in Node.js
- Three.js GLTFLoader API confirmed at `three/examples/jsm/loaders/GLTFLoader.js` in node_modules (v0.183.2)
- JS parser body dimension gap confirmed by reading `parseSource()` in `viewer/src/wasm.ts` — `Partial<ComponentInfo>` cast never sets body fields
- Existing 3D E2E tests in `viewer/e2e/three-d-view.spec.ts` and `viewer/e2e/performance.spec.ts` verified — both pass but don't check geometry counts
- Boundary map requirement: `loadComponentModel(url: string)` method on Renderer3D + confirmed GLTFLoader integration (from M003-ROADMAP.md)
