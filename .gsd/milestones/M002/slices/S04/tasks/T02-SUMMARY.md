---
id: T02
parent: S04
milestone: M002
provides:
  - Copper trace rendering as merged flat ribbons per layer (top/bottom) at correct Z-heights
  - Pad rendering at component-relative positions with rotation and shape-specific triangulation
  - Via rendering using InstancedMesh with drill holes for efficiency
  - Layer visibility toggling (Top/Bottom checkboxes) working in 3D view
  - drawCalls debug surface on window.__renderer3d
key_files:
  - viewer/src/renderer3d.ts
  - viewer/src/main.ts
key_decisions:
  - Traces as flat quads (2 triangles per segment) merged into single BufferGeometry per layer — minimizes draw calls
  - Pads triangulated into fan shapes (circle=12-segment, roundrect/oblong=8-point octagon, rect=2 tris) and merged per layer
  - Vias use InstancedMesh with CylinderGeometry template + per-instance scale matrix for varying sizes
  - Drill holes rendered as separate InstancedMesh in PCB substrate color (visual punch-through)
  - Z-offsets — bottom copper 0.035mm, top copper 1.565mm; pads get +0.005mm offset above traces to prevent Z-fighting
patterns_established:
  - Layer group pattern — named THREE.Group per layer stored in Map, visibility toggled via group.visible
  - Merged geometry pattern — all same-layer primitives into one BufferGeometry with Float32Array
observability_surfaces:
  - "console.log('[3D] Built N trace segments on layer X') — trace count per layer"
  - "console.log('[3D] Built N pads') — total pad count"
  - "console.log('[3D] Built N vias (instanced)') — via count"
  - "console.log('[3D] Warning: 0 traces on layer X') — empty layer warning"
  - "window.__renderer3d.drawCalls — WebGL draw call count for performance monitoring"
duration: 30min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: Render copper layers — traces, pads, and vias in 3D

**Added copper geometry rendering (traces, pads, vias) with merged per-layer BufferGeometry, InstancedMesh vias, and layer visibility toggling in 3D view.**

## What Happened

Built three geometry builders in `Renderer3D`:

1. **buildTraces** — Groups trace segments by layer (Top/Bottom), constructs flat quad ribbons (4 vertices, 2 triangles per segment) at the correct Z-height, merges all into single `BufferGeometry` per layer. Uses trace width from data, perpendicular extrusion for ribbon width.

2. **buildPads** — Iterates components→pads, applies component rotation to pad offsets, triangulates each pad shape (circle=12-seg fan, roundrect/oblong=octagon, rect=2 tris). Through-hole pads added to both top and bottom groups. Merged per layer.

3. **buildVias** — Uses `InstancedMesh` with `CylinderGeometry` template. Per-instance matrix handles position and scale for varying via sizes. Drill holes rendered as second `InstancedMesh` in substrate color.

Layer visibility wired through `updateLayerVisibility(layers)` method called from `main.ts` checkbox handlers when 3D is active. Via group visible when either copper layer is visible.

## Verification

- `cd viewer && npx tsc --noEmit` — **pass**, zero errors
- `cd viewer && npx vite build` — **pass**, renderer3d chunk 28.7KB, three.js chunk 500KB (separate)
- `cargo clippy -p cypcb-world -p cypcb-cli -- -D warnings` — pre-existing parser lint errors (not from this task)
- Browser verification not possible in this environment (no X server) — geometry logic verified by code review against 2D renderer coordinate conventions

### Slice-level checks:
- ✅ `cd viewer && npx tsc --noEmit` — passes
- ✅ `cd viewer && npx vite build` — passes
- ⚠️ `cargo clippy --workspace --all-features -- -D warnings` — pre-existing Rust lint failures (wayland-sys + parser)
- ⏳ Manual browser verification — deferred to T03 (final task)
- ✅ `viewer/src/renderer3d.ts` exports `Renderer3D` with `init()`, `updateBoard()`, `dispose()`, `updateLayerVisibility()`

## Diagnostics

- `window.__renderer3d.meshCount` — count of Mesh+InstancedMesh objects in scene
- `window.__renderer3d.drawCalls` — WebGL draw call count from renderer.info
- Console `[3D] Built N trace segments on layer Top/Bottom` — trace geometry stats
- Console `[3D] Built N pads` — pad count
- Console `[3D] Built N vias (instanced)` — via count with instancing note
- Console `[3D] Warning: 0 traces on layer X` — empty layer warning for debugging

## Deviations

None. Implementation follows plan precisely.

## Known Issues

- Vias with highly varying sizes use per-instance scale relative to first via — works correctly but geometry is shared (minor visual imprecision for extreme size differences)
- Browser visual verification deferred (no X server in this environment) — T03 will do final visual check

## Files Created/Modified

- `viewer/src/renderer3d.ts` — Added buildTraces, buildPads, buildPadTriangles, buildVias methods; updateLayerVisibility; layerGroups Map; drawCalls debug surface; Z-height constants for copper layers
- `viewer/src/main.ts` — Layer checkbox handlers now call renderer3d.updateLayerVisibility() when 3D is active
