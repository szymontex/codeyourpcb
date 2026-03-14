---
estimated_steps: 5
estimated_files: 5
---

# T01: Three.js scene scaffold with 2D/3D toggle and board substrate

**Slice:** S04 — 3D Board Viewer
**Milestone:** M002

## Description

Establishes the foundational Three.js integration into the existing viewer. Creates the `Renderer3D` class with full lifecycle management (init, update, dispose), adds a "3D" toggle button to the toolbar, wires the toggle logic in `main.ts` to lazy-load Three.js on first use, and renders the board substrate as a green PCB slab with correct dimensions from `BoardSnapshot`. Sets up camera, OrbitControls, lighting, theme-synced background, and the `__renderer3d` debug surface.

## Steps

1. **Install Three.js** — `cd viewer && npm install three && npm install -D @types/three`. Update `vite.config.ts` to add `three` to `manualChunks` for code-splitting.

2. **Create `viewer/src/renderer3d.ts`** — Implement `Renderer3D` class with:
   - `init(container: HTMLElement): void` — creates WebGLRenderer, PerspectiveCamera, Scene, OrbitControls, lighting (ambient + directional, RoomEnvironment-style). Sets renderer background from CSS `--bg-canvas`. Starts animation loop. Exposes `window.__renderer3d` debug surface.
   - `updateBoard(snapshot: BoardSnapshot, layers: LayerVisibility): void` — clears existing board geometry, builds board substrate as an `ExtrudeGeometry` box (width × height × 1.6mm thickness) with PCB green material (`MeshPhysicalMaterial`). Converts nm to mm (÷ 1e6). Sets OrbitControls target to board center.
   - `dispose(): void` — disposes all geometries, materials, textures, removes renderer DOM element, stops animation loop, nulls references.
   - `setBackground(color: string): void` — updates scene background from theme color.
   - Private animation loop with `requestAnimationFrame`, only runs when active.
   - Coordinate system: X/Y from board data (in mm), Z is the board stack-up axis (Z-up). Board bottom face at Z=0, top face at Z=1.6mm.

3. **Add 3D toggle button to `viewer/index.html`** — Insert a "3D" button (`#view-3d-btn`) next to the fit button in the toolbar. Style consistent with existing buttons.

4. **Wire 2D↔3D toggle in `viewer/src/main.ts`** — On "3D" button click:
   - First click: dynamic `import('./renderer3d')`, instantiate `Renderer3D`, call `init(container)`.
   - Toggle to 3D: hide 2D canvas, show WebGL canvas, call `updateBoard()` with current snapshot, pause 2D `requestAnimationFrame` loop.
   - Toggle to 2D: show 2D canvas, call `renderer3d.dispose()` or hide WebGL canvas, resume 2D loop.
   - Subscribe to theme changes to call `setBackground()`.
   - Track `is3DActive` boolean state. Show button as active/pressed when 3D mode is on.

5. **Verify build and runtime** — Run `npx tsc --noEmit`, `npx vite build`, confirm Three.js chunk created. Start dev server, load a .cypcb file, click 3D toggle, confirm green board slab appears with orbit/zoom/pan controls, theme toggle changes background.

## Must-Haves

- [ ] Three.js installed and Vite-chunked separately from main bundle
- [ ] `Renderer3D` class with `init()`, `updateBoard()`, `dispose()`, `setBackground()`
- [ ] Board substrate rendered as green slab with correct nm→mm dimensions
- [ ] OrbitControls with orbit target at board center
- [ ] 2D canvas hidden / 2D render loop paused when 3D active
- [ ] Theme-synced background color
- [ ] `window.__renderer3d` debug surface with `{ isActive, meshCount }`
- [ ] Lazy-loaded (no Three.js code in initial bundle)

## Verification

- `cd viewer && npx tsc --noEmit` — TypeScript compiles
- `cd viewer && npx vite build` — build succeeds, output includes a three.js chunk
- Start dev server, open browser, load .cypcb, click 3D → green board substrate visible, mouse orbit works
- Click 3D again → returns to 2D view, canvas renders normally
- `window.__renderer3d.isActive` returns true/false correctly

## Observability Impact

- Signals added/changed: `[3D] Initialized`, `[3D] Board updated: WxH mm, N components`, `[3D] Disposed` console logs
- How a future agent inspects this: `window.__renderer3d` returns `{ isActive, meshCount }` — can be read from browser console or browser_evaluate
- Failure state exposed: WebGL context creation failure logged as `[3D] WebGL not available`, status bar shows error message

## Inputs

- `viewer/src/types.ts` — `BoardSnapshot`, `ComponentInfo` types for data consumption
- `viewer/src/layers.ts` — `LayerVisibility`, `getThemeColors()` for theme integration
- `viewer/src/main.ts` — existing app initialization, render loop, and toolbar wiring
- `viewer/index.html` — existing toolbar HTML structure
- `viewer/vite.config.ts` — existing Vite config with manual chunks

## Expected Output

- `viewer/src/renderer3d.ts` — new file, Three.js 3D renderer class
- `viewer/index.html` — modified, 3D toggle button added to toolbar
- `viewer/src/main.ts` — modified, 2D↔3D toggle wiring with lazy import
- `viewer/vite.config.ts` — modified, Three.js chunk added
- `viewer/package.json` — modified, three + @types/three added
