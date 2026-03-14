# S04: 3D Board Viewer

**Goal:** User can toggle to a 3D view showing the PCB board with procedural component bodies, copper traces, pads, and vias — rendered with Three.js at 60fps with orbit/zoom/pan controls, layer visibility, and theme sync.
**Demo:** User opens a .cypcb file, clicks the "3D" toggle button, sees a 3D board with colored copper layers and component box shapes, orbits/zooms freely, toggles layers on/off, switches themes — all at smooth 60fps.

## Must-Haves

- Three.js WebGL renderer toggleable from the existing toolbar (2D ↔ 3D button)
- Procedural board substrate (green PCB slab at correct dimensions from `BoardSnapshot`)
- Copper layers rendered: traces as extruded paths, pads as extruded shapes, vias as cylinders
- Fallback component bodies as colored boxes matching footprint `bounds` dimensions
- OrbitControls with orbit/zoom/pan, target set to board center
- Layer visibility (Top/Bottom checkboxes) respected in 3D view
- Theme-aware background (syncs with light/dark mode)
- Three.js lazy-loaded via dynamic import (no impact on initial page load)
- Merged geometry per layer for performance (not per-trace meshes)
- Proper dispose() lifecycle when toggling back to 2D
- Extend Rust `ComponentInfo` and TS `ComponentInfo` with `body_width_nm`/`body_height_nm` from footprint bounds
- 60fps on a board with ~100 components (performance baseline)

## Proof Level

- This slice proves: integration
- Real runtime required: yes (browser-rendered Three.js scene consuming real `BoardSnapshot` data from WASM engine)
- Human/UAT required: yes (visual verification of 3D scene quality, orbit behavior, layer toggling)

## Verification

- `cargo clippy --workspace --all-features -- -D warnings` — Rust extension compiles cleanly
- `cd viewer && npx tsc --noEmit` — TypeScript compiles with no errors
- `cd viewer && npx vite build` — production build succeeds (Three.js tree-shakes, chunk created)
- Manual browser verification: load example .cypcb, toggle 3D, confirm scene renders with board substrate + traces + components, orbit with mouse, toggle layers, switch theme
- `viewer/src/renderer3d.ts` exists and exports `Renderer3D` class with `init()`, `updateBoard()`, `dispose()` methods

## Observability / Diagnostics

- Runtime signals: `console.log('[3D]', ...)` prefix for all 3D renderer lifecycle events (init, updateBoard, dispose, model count, geometry stats)
- Inspection surfaces: `window.__renderer3d` debug object exposing `{ isActive, meshCount, drawCalls, fps }` when 3D is active
- Failure visibility: WebGL context loss logged with `[3D] WebGL context lost` + auto-fallback to 2D view; Three.js load failure caught and surfaced in status bar
- Redaction constraints: none (no secrets in rendering pipeline)

## Integration Closure

- Upstream surfaces consumed: `BoardSnapshot` from `PcbEngine.get_snapshot()` (same data as 2D renderer), `LayerVisibility` from `layers.ts`, theme system from `theme/theme-manager.ts`, toolbar in `index.html`
- New wiring introduced in this slice: 3D toggle button in toolbar → lazy-loads `renderer3d.ts` → creates Three.js scene in `#canvas-container` → consumes same `BoardSnapshot` as 2D renderer → pauses 2D `requestAnimationFrame` loop when active → disposes on toggle back
- What remains before the milestone is truly usable end-to-end: GLB model loading for real component shapes (stretch/S06), DSL v2 (S05), E2E tests (S07), performance polish (S08)

## Tasks

- [x] **T01: Three.js scene scaffold with 2D/3D toggle and board substrate** `est:2h`
  - Why: Establishes the foundational Three.js integration — lazy loading, WebGL canvas, camera, OrbitControls, board substrate geometry, and the 2D↔3D toggle lifecycle. Everything else builds on this working.
  - Files: `viewer/src/renderer3d.ts`, `viewer/index.html`, `viewer/src/main.ts`, `viewer/vite.config.ts`, `viewer/package.json`
  - Do: Install three.js, create `Renderer3D` class with `init(container)` / `updateBoard(snapshot, layers)` / `dispose()`. Add "3D" toggle button to toolbar. Wire toggle in `main.ts` to lazy-import renderer3d, create scene with camera + OrbitControls + lighting + board substrate slab. Pause 2D render loop when 3D active. Theme-sync background. Debug surface `window.__renderer3d`. Configure Vite to chunk Three.js separately.
  - Verify: `npx tsc --noEmit` passes, `npx vite build` creates three.js chunk, clicking 3D toggle in browser shows green board substrate with orbit controls
  - Done when: 3D toggle shows/hides a Three.js scene with correct board dimensions, orbit works, 2D canvas pauses

- [x] **T02: Render copper layers — traces, pads, and vias in 3D** `est:2h`
  - Why: The board substrate alone isn't useful — users need to see actual copper geometry. This is the core rendering content that makes the 3D view valuable. Merged geometry per layer for performance.
  - Files: `viewer/src/renderer3d.ts`, `viewer/src/layers.ts`
  - Do: Build trace geometry as merged `BufferGeometry` per layer (flat ribbons at copper Z-height with trace width). Build pad geometry from `ComponentInfo.pads` (extruded shapes at correct positions with component rotation). Build via geometry using `InstancedMesh` (cylinder + drill hole). Color-code by layer (reuse `LAYER_COLORS`). Respect `LayerVisibility` — show/hide layer groups. Use explicit Z-offsets to prevent Z-fighting (bottom copper = 0.01mm, top copper = board_thickness - 0.01mm).
  - Verify: `npx tsc --noEmit` passes, browser shows traces as colored ribbons on board, pads visible at component positions, vias as cylinders, toggling Top/Bottom layer checkboxes shows/hides corresponding 3D geometry
  - Done when: All copper features from `BoardSnapshot` render correctly in 3D with layer visibility control

- [x] **T03: Component bodies, footprint bounds extension, and integration polish** `est:2h`
  - Why: Components are just pads without visible bodies — users need to see component outlines to understand board layout in 3D. Also needs the Rust-side data extension to provide body dimensions, plus final polish (keyboard shortcut, performance logging, dispose lifecycle verification).
  - Files: `crates/cypcb-render/src/snapshot.rs`, `crates/cypcb-render/src/lib.rs`, `viewer/src/types.ts`, `viewer/src/renderer3d.ts`, `viewer/src/main.ts`
  - Do: Extend Rust `ComponentInfo` with `body_width_nm: i64` and `body_height_nm: i64` populated from `Footprint.bounds`. Update TS `ComponentInfo` type to match. Build component bodies as colored boxes (height ~1-2mm for SMD, ~5mm for THT based on footprint type). Position with rotation. Add refdes label as 3D text sprite. Wire keyboard shortcut (e.g. `3` key) for 3D toggle. Add geometry stats to `__renderer3d` debug surface. Verify dispose() cleans up all GPU resources.
  - Verify: `cargo clippy --workspace --all-features -- -D warnings` passes, `npx tsc --noEmit` passes, `npx vite build` succeeds, browser shows component boxes at correct positions/rotations with refdes labels, `window.__renderer3d.meshCount` returns a reasonable number
  - Done when: 3D view shows complete board with substrate + copper + component bodies, all verification commands pass, dispose lifecycle confirmed clean

## Files Likely Touched

- `viewer/src/renderer3d.ts` (new — Three.js 3D renderer)
- `viewer/src/main.ts` (2D/3D toggle wiring)
- `viewer/index.html` (3D toggle button in toolbar)
- `viewer/vite.config.ts` (Three.js chunk config)
- `viewer/package.json` (three dependency)
- `viewer/src/types.ts` (ComponentInfo body dimensions)
- `viewer/src/layers.ts` (possible color export additions)
- `crates/cypcb-render/src/snapshot.rs` (ComponentInfo body dimensions)
- `crates/cypcb-render/src/lib.rs` (populate body dimensions from footprint bounds)
