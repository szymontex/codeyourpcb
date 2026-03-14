---
estimated_steps: 4
estimated_files: 2
---

# T02: Render copper layers — traces, pads, and vias in 3D

**Slice:** S04 — 3D Board Viewer
**Milestone:** M002

## Description

Builds the core copper geometry rendering that makes the 3D view actually useful. Traces are rendered as flat ribbons (merged `BufferGeometry` per layer) at the correct copper Z-height. Pads are rendered as extruded shapes at component positions with rotation applied. Vias are rendered using `InstancedMesh` for efficiency. All geometry respects `LayerVisibility` — toggling Top/Bottom checkboxes shows/hides the corresponding 3D groups. Colors match the existing `LAYER_COLORS` from the 2D renderer.

## Steps

1. **Build trace geometry** — In `renderer3d.ts`, add a `buildTraces(traces: TraceInfo[], layer: string, zHeight: number)` method. For each layer, merge all trace segments into a single `BufferGeometry`:
   - Each trace segment becomes a flat quad (4 vertices, 2 triangles) with width = `trace.width` (nm→mm) and positioned at the segment's start/end coordinates.
   - Use `Float32Array` for position attributes, build manually for maximum merge efficiency.
   - Color by layer using `LAYER_COLORS.top_copper` / `LAYER_COLORS.bottom_copper` parsed to Three.js `Color`.
   - Z-heights: bottom copper = 0.035mm (copper thickness), top copper = 1.565mm (1.6mm - 0.035mm).
   - Create one `Mesh` per layer, add to a `layerGroups` Map keyed by layer name for visibility toggling.

2. **Build pad geometry** — Add `buildPads(components: ComponentInfo[])` method:
   - For each component, for each pad: create a small extruded shape (rect, circle, roundrect, oblong) at the pad's world position (component position + rotated pad offset).
   - Merge all top-layer pads into one geometry, all bottom-layer pads into another.
   - Through-hole pads get added to both groups.
   - Copper color per layer, slight Z-offset above traces (top pads at 1.57mm, bottom pads at 0.03mm) to prevent Z-fighting with traces.
   - Pad thickness: 0.035mm (copper thickness).

3. **Build via geometry** — Add `buildVias(vias: ViaInfo[])` method:
   - Use `InstancedMesh` with a cylinder geometry template (outer_diameter radius, board thickness height).
   - Set per-instance matrix (position) and color (gray, matching `LAYER_COLORS.via`).
   - Drill hole: use a second `InstancedMesh` with smaller cylinder in background color, or use `CylinderGeometry` with inner radius (tube geometry).
   - Vias span full board thickness (Z=0 to Z=1.6mm).

4. **Wire layer visibility** — Modify `updateBoard()` to create named `Group` objects per layer. In a new `updateLayerVisibility(layers: LayerVisibility)` method, set `group.visible = true/false` based on `layers.topCopper` / `layers.bottomCopper`. Hook this into the existing layer checkbox change handlers in `main.ts` so toggling layers updates both 2D and 3D views.

## Must-Haves

- [ ] Traces rendered as colored ribbons with correct width and position, merged per layer
- [ ] Pads rendered at correct component-relative positions with rotation
- [ ] Vias rendered as cylinders spanning board thickness
- [ ] Layer visibility controls (Top/Bottom checkboxes) work in 3D view
- [ ] Z-offsets prevent Z-fighting between copper layers and substrate
- [ ] Colors match existing `LAYER_COLORS` (red = top, blue = bottom, gray = via)
- [ ] Performance: merged geometry, not per-trace meshes

## Verification

- `cd viewer && npx tsc --noEmit` — TypeScript compiles
- Browser: load a routed .cypcb (e.g. `examples/routing-test.cypcb` with routes), toggle 3D → traces visible as colored ribbons on board surface
- Browser: pads visible at component positions, matching 2D view layout
- Browser: vias appear as gray cylinders at via positions
- Browser: uncheck "Top" layer → top copper traces/pads disappear, bottom remains; re-check → reappears
- `window.__renderer3d.meshCount` returns reasonable count (not thousands)

## Observability Impact

- Signals added/changed: `[3D] Built N trace segments on layer X`, `[3D] Built N pads`, `[3D] Built N vias (instanced)` console logs with geometry counts
- How a future agent inspects this: `window.__renderer3d.meshCount` and `window.__renderer3d.drawCalls` show rendering efficiency
- Failure state exposed: empty geometry logged as `[3D] Warning: 0 traces on layer X`

## Inputs

- `viewer/src/renderer3d.ts` — T01's Renderer3D class with scene, camera, board substrate
- `viewer/src/types.ts` — `TraceInfo`, `ViaInfo`, `ComponentInfo`, `PadInfo` for geometry data
- `viewer/src/layers.ts` — `LAYER_COLORS`, `LayerVisibility` for colors and visibility state
- `viewer/src/main.ts` — existing layer checkbox handlers, 3D toggle wiring from T01

## Expected Output

- `viewer/src/renderer3d.ts` — modified, copper geometry builders added (traces, pads, vias, layer visibility)
- `viewer/src/main.ts` — modified, layer checkbox handlers updated to call `updateLayerVisibility()` when 3D active (if not already reactive)
