# S02: 3D View Fix & Component Rendering — UAT

**Milestone:** M003
**Written:** 2026-03-13

## UAT Type

- UAT mode: mixed (artifact-driven E2E tests + live-runtime debug surface)
- Why this mode is sufficient: Debug surface counters provide deterministic proof of rendered geometry. Pixel comparison is unreliable in headless WebGL (per existing decision). Visual spot-check covers material/color quality that counters can't prove.

## Preconditions

- Dev server running (`cd viewer && npm run dev`)
- Browser with WebGL support
- blink.cypcb available in the default editor content

## Smoke Test

Load app → click 3D toggle → dark gray and tan component boxes visible on the green board substrate. Browser console `__renderer3d.componentCount` returns 9.

## Test Cases

### 1. Component bodies appear in 3D view

1. Open app at localhost:5173
2. Ensure blink.cypcb is loaded in the editor
3. Click the 3D toggle button (or press `3`)
4. **Expected:** Component bodies visible as colored boxes on the PCB substrate — dark gray for ICs, tan for passives. Not an empty green board.

### 2. Debug surface reports geometry counts

1. With 3D active and blink.cypcb loaded, open browser console
2. Run `window.__renderer3d`
3. **Expected:** Object with `componentCount: 9`, `padCount: 24`, `meshCount: 12`, `isActive: true`, and numeric values for `traceSegmentCount` and `viaCount`

### 3. Re-toggle preserves geometry

1. With 3D active, click 2D to go back
2. Click 3D again
3. Run `window.__renderer3d.componentCount` in console
4. **Expected:** Same componentCount as before (9). Geometry fully reconstructed after round-trip.

### 4. E2E test suite passes

1. Run `cd viewer && npx playwright test e2e/three-d-view.spec.ts`
2. **Expected:** 6 tests pass (3 existing toggle tests + 3 new geometry tests)

### 5. No performance regression

1. Run `cd viewer && npx playwright test e2e/performance.spec.ts`
2. **Expected:** FPS ≥30, load time under 3000ms

## Edge Cases

### Empty board (no components)

1. Clear the editor content, type a minimal board with no components
2. Toggle 3D
3. **Expected:** Green substrate visible, componentCount = 0, no errors in console

### Rapid toggle stress

1. Click 3D/2D toggle rapidly 10 times
2. **Expected:** No WebGL errors, no memory leak warnings, final state matches last toggle direction

## Failure Signals

- componentCount === 0 with blink.cypcb loaded → parse pipeline regression
- Console NaN warnings → body dimension computation broken
- WebGL context lost errors → dispose/rebuild cycle leaking resources
- Invisible meshes but meshCount > 0 → NaN guard regression (geometry created but zero-size)

## Requirements Proved By This UAT

- None newly proved — 3D board preview was already validated. This UAT proves the fix to an existing broken capability.

## Not Proven By This UAT

- GLB model loading from real URLs (no models available until S06)
- Material/lighting quality for loaded GLTF scenes
- 3D rendering of copper fill zones (Zone type doesn't exist in data model)

## Notes for Tester

- The 3D view uses procedural box geometry for components — they look like colored blocks, not realistic IC packages. This is expected. Real 3D models come in S06.
- If componentCount shows 0, check the browser console for `[3D] Warning:` messages — they indicate which components failed dimension computation.
