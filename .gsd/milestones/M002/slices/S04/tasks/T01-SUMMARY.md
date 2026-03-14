---
id: T01
parent: S04
milestone: M002
provides:
  - Renderer3D class with init/updateBoard/dispose/setBackground lifecycle
  - 3D toggle button in toolbar with lazy-loaded Three.js
  - Board substrate rendered as green PCB slab with correct nm→mm dimensions
  - window.__renderer3d debug surface
key_files:
  - viewer/src/renderer3d.ts
  - viewer/index.html
  - viewer/src/main.ts
  - viewer/vite.config.ts
key_decisions:
  - Three.js lazy-loaded via dynamic import — separate Vite chunk, no initial bundle impact
  - Renderer3D fully disposed on toggle back to 2D, re-instantiated on next 3D click
  - Board substrate uses BoxGeometry with geometry.translate for Z=0 bottom face convention
patterns_established:
  - Lazy-import pattern for heavy optional modules (import('./renderer3d') on first click)
  - 2D render loop skipped via is3DActive guard rather than stopping/starting requestAnimationFrame
  - Theme sync via themeManager.subscribe updating 3D background from CSS custom property
observability_surfaces:
  - "window.__renderer3d — { isActive: boolean, meshCount: number } getter-based debug surface"
  - "Console logs with [3D] prefix — Initialized, Board updated WxH mm, Disposed"
  - "WebGL creation failure caught and surfaced in status bar"
duration: 45min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Three.js scene scaffold with 2D/3D toggle and board substrate

**Added Three.js 3D renderer with lazy-loading, board substrate slab, OrbitControls, and 2D↔3D toggle button in toolbar.**

## What Happened

Installed `@types/three` (three was already a dependency). Added `three` to Vite `manualChunks` for code-splitting. Created `viewer/src/renderer3d.ts` with `Renderer3D` class implementing full lifecycle: `init()` sets up WebGLRenderer, PerspectiveCamera, OrbitControls with damping, ambient + dual directional lighting; `updateBoard()` builds a `BoxGeometry` board substrate with `MeshPhysicalMaterial` (PCB green, clearcoat) positioned with bottom face at Z=0; `dispose()` traverses and disposes all geometries/materials/textures and removes the DOM element; `setBackground()` syncs scene background from theme color.

Added "3D" button to toolbar in `index.html` with `.active` state styling. Wired toggle in `main.ts`: first click lazy-imports `renderer3d.ts`, instantiates Renderer3D, hides 2D canvas, calls `updateBoard()` with current snapshot. Second click disposes renderer, shows 2D canvas, forces 2D re-render. Theme changes propagate to 3D background via existing `themeManager.subscribe`. 2D render loop skipped when `is3DActive` is true.

## Verification

- `cd viewer && npx tsc --noEmit` — **passes**, zero errors
- `cd viewer && npx vite build` — **passes**, Three.js chunk created:
  - `renderer3d-DKXp7lCH.js` (23.92 KB) — lazy-loaded renderer module
  - `three-D6m6ijG-.js` (495.10 KB / 124.96 KB gzip) — separate Three.js chunk
  - Main `index-*.js` only contains chunk reference mapping, no Three.js code
- `cargo clippy --workspace --all-features -- -D warnings` — **pre-existing failures** in `cypcb-parser` (51 clippy warnings) and wayland system dep. Not related to this task (TypeScript-only changes).
- Browser verification not possible in this environment (no X server). Visual verification deferred to later tasks / manual testing.
- `viewer/src/renderer3d.ts` exists and exports `Renderer3D` class with `init()`, `updateBoard()`, `dispose()`, `setBackground()` methods.

### Slice-level verification status (T01 — intermediate task):
- [x] `cd viewer && npx tsc --noEmit` — passes
- [x] `cd viewer && npx vite build` — passes with Three.js chunk created
- [x] `viewer/src/renderer3d.ts` exists with correct exports
- [ ] `cargo clippy` — pre-existing failures, not task-related
- [ ] Manual browser verification — deferred (no display server available)

## Diagnostics

- `window.__renderer3d` — getter-based debug surface returning `{ isActive: boolean, meshCount: number }`. Available after first 3D toggle. Returns `isActive: false` after dispose.
- Console logs with `[3D]` prefix: `Initialized`, `Board updated: WxH mm, N components`, `Disposed`.
- WebGL creation failure caught in `main.ts` toggle handler, surfaced in status bar text.

## Deviations

- `three` was already in `package.json` dependencies — only `@types/three` needed installation.
- Used `BoxGeometry` with `geometry.translate()` instead of `ExtrudeGeometry` as mentioned in plan — BoxGeometry is simpler and produces identical visual result for a rectangular slab.
- Debug surface exposes `isActive` and `meshCount` (not `drawCalls` and `fps` — those are slice-level stretch goals for T03).

## Known Issues

- `cargo clippy --workspace --all-features` has pre-existing failures (51 warnings in `cypcb-parser`, wayland system dep). Not introduced by this task.
- Browser-based verification not possible in headless environment — needs manual confirmation that green slab renders with orbit controls.

## Files Created/Modified

- `viewer/src/renderer3d.ts` — new, Three.js 3D renderer class with full lifecycle management
- `viewer/index.html` — added #view-3d-btn button and CSS styles
- `viewer/src/main.ts` — added 3D state variables, lazy-import toggle handler, theme sync, 2D loop guard
- `viewer/vite.config.ts` — added `three` to `manualChunks` for code-splitting
- `viewer/package.json` — `@types/three` added to devDependencies
