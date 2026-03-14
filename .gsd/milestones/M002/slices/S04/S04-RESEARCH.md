# S04: 3D Board Viewer — Research

**Date:** 2026-03-13

## Summary

The 3D board viewer needs to render the PCB board, traces, pads, vias, and component bodies in an interactive Three.js scene at 60fps with orbit/zoom/pan controls. The existing codebase has everything needed on the data side — `BoardSnapshot` already contains full geometry (board dimensions, component positions + footprint pads, trace segments, vias). The current 2D Canvas renderer (`renderer.ts`) is well-structured and gives a clear template for what to draw.

The primary architectural decision is the model pipeline. Atopile depends on KiCad CLI (`kicad-cli pcb export glb`) to export entire boards as GLB — this violates our standalone constraint. Instead, we'll build the board geometry procedurally in Three.js (extruded board substrate, traces as extruded paths, pads as extruded shapes) and load component 3D models (GLB/GLTF) when available from a model cache. Fallback is procedural box/cylinder shapes matching footprint bounds. This keeps the tool fully standalone while producing professional output.

The STEP-to-GLB conversion for JLCPCB models should happen offline (build-time or CLI tool), not in-browser. OpenCascade.js can do it in-browser but adds 2.4-9.1MB to the bundle — unacceptable for our <3s load time target. A CLI converter using OpenCascade or FreeCAD can pre-convert STEP files from the CDFER/JLCPCB-Kicad-Library (which already has STEP models for common JLCPCB parts) into optimized GLB assets served from a static cache.

## Recommendation

**Phased approach:**

1. **Procedural 3D board** — generate board substrate, copper layers (traces, pads, vias) directly from `BoardSnapshot` data in Three.js. Fallback component bodies as colored boxes matching footprint `bounds`. This alone produces a useful, professional-looking 3D view.

2. **GLTF model loading** — extend `ComponentInfo` (both Rust snapshot and TS types) with an optional `model_3d` field. Load GLB models via Three.js `GLTFLoader` from a model cache directory. Models keyed by footprint name or LCSC part number.

3. **Model pipeline CLI** — build a `cypcb-cli model-convert` command that batch-converts STEP files to optimized GLB using FreeCAD or OpenCASCADE headless. Pre-convert the JLCPCB library. Serve from `public/models/` or CDN.

Phase 1 is the slice deliverable. Phases 2-3 can start in this slice but are stretch goals.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| 3D scene/rendering | Three.js (three@0.162+) | Industry standard, GLTFLoader, OrbitControls, MeshPhysicalMaterial — exactly what atopile uses successfully |
| Orbit/zoom/pan controls | Three.js OrbitControls | Battle-tested, supports damping, touch, constraints |
| GLTF/GLB model loading | Three.js GLTFLoader + DRACOLoader | Handles compressed models, PBR materials, standard format |
| Realistic lighting | Three.js RoomEnvironment + PMREMGenerator | Studio-quality environment map without external HDR download |
| STEP to GLB conversion | FreeCAD headless or OpenCASCADE CLI | Offline conversion avoids browser bundle bloat |
| Extrude 2D shapes to 3D | Three.js ExtrudeGeometry / ShapeGeometry | Built-in, handles PCB profile and trace cross-sections |

## Existing Code and Patterns

- `viewer/src/renderer.ts` — Canvas 2D renderer with `RenderState`, layer-based draw ordering (bottom traces → top traces → components → vias → ratsnest → violations). The 3D renderer should follow the same data flow: consume `BoardSnapshot`, respect `LayerVisibility`.
- `viewer/src/types.ts` — `BoardSnapshot`, `ComponentInfo`, `TraceInfo`, `ViaInfo` types. These are the primary data source for 3D geometry generation. All coordinates in nanometers.
- `viewer/src/viewport.ts` — 2D viewport with world↔screen coordinate transforms. 3D camera should use the same world coordinate origin (board corner at 0,0) but converted to meters or mm for Three.js.
- `viewer/src/layers.ts` — `LayerVisibility`, `LAYER_COLORS`, `netColor()`. 3D view should reuse these colors and respect layer toggling.
- `viewer/src/main.ts` — Main app entry. The 3D view needs a toggle button in the toolbar (2D ↔ 3D) that swaps the canvas for a Three.js WebGL canvas. Editor panel, status bar, and toolbar remain shared.
- `viewer/index.html` — Toolbar with layer checkboxes, theme toggle, etc. Add a "3D" toggle button next to the "Fit" button.
- `crates/cypcb-render/src/snapshot.rs` — Rust `BoardSnapshot` struct. `ComponentInfo` has `footprint` name and `pads` but no `model_3d` field yet.
- `crates/cypcb-world/src/footprint/library.rs` — `Footprint` struct has `bounds: Rect` and `courtyard: Rect`. These give component body dimensions for procedural fallback shapes.
- `viewer/vite.config.ts` — Vite config with WASM plugin, Monaco chunking. Three.js should get its own chunk for code-splitting.
- Atopile reference: `/workspace/competitors/atopile/src/vscode-atopile/src/ui/modelviewer.ts` — Their Three.js viewer pattern: scene + camera + OrbitControls + GLTFLoader + MeshPhysicalMaterial + RoomEnvironment. Good reference for materials and lighting setup.

## Constraints

- **Standalone**: No KiCad dependency. Board geometry must be generated from `BoardSnapshot` data, not exported via `kicad-cli`.
- **Coordinate system**: Board data is in nanometers (i64). Three.js works best with values near 1.0. Convert nm to mm (divide by 1e6) for the 3D scene to keep values in a reasonable range.
- **Y-axis flip**: Board world is Y-up, Three.js is Y-up — but the 2D renderer flips Y for screen. The 3D renderer should use Y-up directly (no flip needed), with Z as the board stack-up axis.
- **Performance target**: 60fps for a board with ~500 components. Key strategies: merged geometry (single BufferGeometry for all traces on a layer), instanced meshes for vias, frustum culling. Avoid creating thousands of individual Mesh objects.
- **Bundle size**: Three.js tree-shakes well with ES module imports. The core + OrbitControls + GLTFLoader should add ~150-200KB gzipped. Acceptable.
- **Theme sync**: Background color and any UI elements must respect the light/dark theme system (CSS custom properties, `themeManager.subscribe()`).
- **Existing UI integration**: The 3D view replaces the 2D canvas when toggled. It must share the same container (`#canvas-container`), status bar, and toolbar. The 2D `requestAnimationFrame` loop pauses when 3D is active.
- **WASM compatibility**: Three.js is pure JS — no WASM interaction needed for the 3D renderer itself. The data comes from the same `engine.get_snapshot()` call.
- **Web load time <3s target**: Three.js must be lazy-loaded (dynamic import) so it doesn't block initial page load. Only load when user first clicks the 3D toggle.

## Common Pitfalls

- **Creating too many Three.js objects** — each `Mesh` has draw call overhead. A board with 200 traces × 5 segments = 1000 meshes will kill framerate. Merge trace geometry per layer into a single `BufferGeometry` using `mergeBufferGeometries` or manual attribute concatenation.
- **Z-fighting between copper layers** — board substrate, bottom copper, top copper, solder mask, and silkscreen are all very thin and close together. Use explicit Z offsets (e.g., board=0, bottom copper=0.01mm, top copper=1.6mm-0.01mm) and enable `logarithmicDepthBuffer` if needed.
- **Coordinate scale issues** — nm values are huge integers (1mm = 1,000,000). Passing raw nm to Three.js causes floating-point precision issues at large coordinates. Always convert to mm before creating geometry.
- **Memory leaks on view toggle** — Three.js geometries, materials, and textures must be explicitly `.dispose()`d when switching back to 2D view. A `dispose()` method on the 3D renderer is essential.
- **OrbitControls target mismatch** — The orbit target should be set to the board center, not origin. Otherwise zooming/rotating feels off-center.
- **Theme background flash** — Initialize the Three.js renderer with the correct background color from CSS custom properties before the first frame renders, just like the 2D renderer does.
- **GLTFLoader caching** — Without caching, the same 0402 resistor model loads 50 times for 50 resistors. Use a model cache map keyed by footprint name, loading each model once and cloning for instances.

## Open Risks

- **Footprint bounds accuracy** — The `Footprint.bounds` Rect is the component body outline. For procedural 3D shapes, this determines box height/width. If bounds are wrong, components look wrong. Need to verify bounds are populated for all built-in footprints.
- **Performance with large boards** — 500 components with traces may push draw call limits. May need geometry instancing (InstancedMesh for repeated footprints like 0402) and LOD (simplified geometry at distance).
- **STEP model availability** — The CDFER/JLCPCB-Kicad-Library has STEP models for ~6000 basic/preferred parts, but custom or rare parts won't have models. The procedural fallback must look acceptable as the default experience.
- **Board stack-up representation** — Currently `BoardInfo.layer_count` is the only stack-up info. For accurate 3D, we need substrate thickness (typically 1.6mm for 2-layer), copper thickness (35µm), and solder mask thickness. Hardcode 1.6mm for now, parameterize later.
- **WebGL compatibility** — Three.js WebGLRenderer should work on all target browsers (Chrome, Firefox, Safari, Edge). WebGPU renderer is newer but not universally supported yet — stick with WebGL for now.

## Requirements Mapping

| Requirement | How This Slice Addresses It |
|---|---|
| LIB-03: User can associate 3D STEP models with components | Extends `ComponentInfo` with optional `model_3d` field; GLTFLoader renders associated models |
| (implicit from roadmap) 3D viewer at 60fps with orbit/zoom | Procedural board geometry + Three.js OrbitControls with performance optimization |
| (implicit from roadmap) Layer visibility in 3D | Layer toggle controls which copper layers/components are visible in 3D scene |
| UI-05: Theme applies consistently | 3D renderer background syncs with light/dark theme |

## Architecture Sketch

```
┌──────────────────────────────────────────┐
│ main.ts                                  │
│  ┌─────────────┐    ┌─────────────────┐  │
│  │ 2D Renderer │ OR │ 3D Renderer     │  │
│  │ (Canvas 2D) │    │ (Three.js WebGL)│  │
│  └──────┬──────┘    └───────┬─────────┘  │
│         │                   │            │
│         └───────┬───────────┘            │
│                 │                        │
│          BoardSnapshot                   │
│         (from PcbEngine)                 │
└──────────────────────────────────────────┘

3D Renderer internals:
┌────────────────────────────────────────┐
│ renderer3d.ts                          │
│  - init(container): Scene/Camera/Ctrl  │
│  - updateBoard(snapshot, layers): void │
│  - dispose(): cleanup                  │
│                                        │
│  Board geometry builder:               │
│   - buildSubstrate(boardInfo)          │
│   - buildTraces(traces[], layer)       │
│   - buildPads(components[])            │
│   - buildVias(vias[])                  │
│   - buildComponents(components[])      │
│                                        │
│  Model loader:                         │
│   - loadModel(footprint): Promise<Grp> │
│   - modelCache: Map<string, Group>     │
└────────────────────────────────────────┘
```

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Three.js | `cloudai-x/threejs-skills@threejs-fundamentals` | available (2K installs) |
| Three.js | `cloudai-x/threejs-skills@threejs-geometry` | available (1.6K installs) |
| Three.js | `cloudai-x/threejs-skills@threejs-shaders` | available (1.7K installs) |
| WebGL | `martinholovsky/claude-skills-generator@webgl` | available (157 installs) |

The `threejs-fundamentals` and `threejs-geometry` skills are directly relevant and may be worth installing for the implementation phase.

## Sources

- Atopile uses KiCad CLI `pcb export glb` for 3D, then Three.js GLTFLoader with MeshPhysicalMaterial + RoomEnvironment + DRACOLoader (source: `/workspace/competitors/atopile/src/vscode-atopile/src/ui/modelviewer.ts`)
- JLCPCB 3D models available via CDFER/JLCPCB-Kicad-Library on GitHub with STEP files for basic/preferred parts (source: Google Search — github.com/CDFER/JLCPCB-Kicad-Library)
- OpenCascade.js can convert STEP to mesh in browser (2.4MB compressed custom build) but has performance concerns with `TransferRoots()` (source: Google Search — ocjs.org, medium.com)
- Three.js PCB visualization: ExtrudeGeometry for traces, merged BufferGeometry for performance, procedural board layers with Z-offset (source: Google Search — github.com/jglim/PurpleVisualizer)
- Three.js OrbitControls + GLTFLoader + PMREMGenerator pattern is the standard for product viewers (source: Three.js examples via Context7)
