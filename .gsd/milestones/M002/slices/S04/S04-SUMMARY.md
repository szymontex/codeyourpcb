---
id: S04
parent: M002
milestone: M002
provides:
  - Three.js 3D board viewer with lazy-loaded WebGL renderer
  - Procedural board substrate at correct nm→mm dimensions
  - Copper layer rendering (traces as merged flat ribbons, pads as triangulated shapes, vias as InstancedMesh)
  - Component body boxes with SMD/THT height differentiation and refdes sprite labels
  - OrbitControls with orbit/zoom/pan
  - Layer visibility toggling in 3D view
  - Theme-synced background (light/dark)
  - Keyboard shortcut '3' for 3D toggle
  - Rust ComponentInfo extended with body_width_nm/body_height_nm/model_3d
  - window.__renderer3d debug surface with isActive, meshCount, drawCalls, fps
requires:
  - slice: S03
    provides: Upgraded 2D renderer with BoardSnapshot data model, LayerVisibility, toolbar infrastructure
affects:
  - S06 (GLB model loading for real component shapes)
  - S07 (E2E test coverage of 3D toggle/orbit/layer interactions)
  - S08 (60fps performance verification)
key_files:
  - viewer/src/renderer3d.ts
  - viewer/src/main.ts
  - viewer/index.html
  - viewer/vite.config.ts
  - viewer/src/types.ts
  - crates/cypcb-render/src/snapshot.rs
  - crates/cypcb-render/src/lib.rs
key_decisions:
  - Three.js lazy-loaded via dynamic import — separate Vite chunk, zero initial bundle impact
  - Renderer3D fully disposed on toggle back to 2D, re-instantiated on next 3D click
  - Board substrate uses BoxGeometry with translate for bottom-face-at-Z=0 convention
  - Traces as flat quads (2 tris per segment) merged into single BufferGeometry per layer — minimizes draw calls
  - Vias use InstancedMesh with per-instance scale matrix; drill holes as second InstancedMesh in substrate color
  - Z-offsets for copper layers (bottom=0.035mm, top=board_thickness-0.035mm, pads +0.005mm above traces) prevent Z-fighting
  - Component height heuristic — SMD=1.2mm, THT=5mm, detected by presence of drill pads
  - Refdes labels as THREE.Sprite with CanvasTexture for camera-facing readability
  - Pad bbox fallback when footprint bounds are zero — computes from pad extents
patterns_established:
  - Lazy-import pattern for heavy optional modules (import('./renderer3d') on first click)
  - 2D render loop skipped via is3DActive guard rather than stopping/starting requestAnimationFrame
  - Layer group pattern — named THREE.Group per layer in Map, visibility toggled via group.visible
  - Merged geometry pattern — all same-layer primitives into one BufferGeometry with Float32Array
  - Sprite label pattern — canvas → CanvasTexture → SpriteMaterial → Sprite for 3D text
observability_surfaces:
  - "window.__renderer3d — { isActive, meshCount, drawCalls, fps } getter-based debug surface"
  - "Console logs with [3D] prefix — Initialized, Board updated, Built N traces/pads/vias/components, FPS every 5s, Disposed"
  - "WebGL creation failure caught and surfaced in status bar"
drill_down_paths:
  - .gsd/milestones/M002/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M002/slices/S04/tasks/T02-SUMMARY.md
  - .gsd/milestones/M002/slices/S04/tasks/T03-SUMMARY.md
duration: 1h40min
verification_result: passed
completed_at: 2026-03-13
---

# S04: 3D Board Viewer

**Three.js 3D board viewer with procedural copper geometry, component bodies, orbit controls, and layer visibility — lazy-loaded with zero initial bundle impact.**

## What Happened

Built a complete 3D board visualization pipeline in three tasks. T01 established the foundation: Three.js integration via dynamic import (separate 508KB gzip-128KB chunk), `Renderer3D` class with full lifecycle (`init`/`updateBoard`/`dispose`/`setBackground`), OrbitControls with damping, ambient+directional lighting, green PCB substrate slab sized from `BoardSnapshot` nm dimensions. 2D↔3D toggle button in toolbar with lazy instantiation on first click and full disposal on toggle-back.

T02 added copper geometry rendering. Traces rendered as flat quad ribbons (2 triangles per segment) merged into single `BufferGeometry` per layer for minimal draw calls. Pads triangulated by shape (circle=12-segment fan, roundrect=octagon, rect=2 tris) with component rotation applied, merged per layer. Vias use `InstancedMesh` with per-instance scale matrix — drill holes as separate `InstancedMesh` in substrate color for visual punch-through. Layer visibility wired through named `THREE.Group` objects toggled from Top/Bottom checkboxes.

T03 extended Rust `ComponentInfo` with `body_width_nm`/`body_height_nm`/`model_3d` fields, populated from `Footprint.bounds` with a pad-bbox fallback. Component bodies rendered as colored `BoxGeometry` (dark gray metallic for ICs, tan for passives) with SMD/THT height differentiation. Refdes labels as camera-facing `Sprite` objects with `CanvasTexture`. Added `3` keyboard shortcut, FPS tracking (logged every 5s), and full `{ isActive, meshCount, drawCalls, fps }` debug surface.

## Verification

- `cd viewer && npx tsc --noEmit` — **pass**, zero errors
- `cd viewer && npx vite build` — **pass**; `renderer3d` chunk 30.89KB/8.47KB gzip, `three` chunk 507.63KB/127.70KB gzip, main bundle contains no Three.js code
- `cargo check -p cypcb-render --all-features` — **pass**, compiles cleanly
- `cargo test -p cypcb-render --all-features` — **pass**, 33 tests (including `test_component_body_dimensions_from_footprint`)
- `viewer/src/renderer3d.ts` exports `Renderer3D` class with `init()`, `updateBoard()`, `dispose()`, `updateLayerVisibility()`, `setBackground()` methods
- `cargo clippy --workspace --all-features -- -D warnings` — pre-existing failures in `cypcb-parser` (51 warnings) and wayland system dep; not introduced by S04
- Manual browser verification deferred — headless CI environment lacks X server

## Requirements Advanced

- LIB-03 (3D STEP models) — Rust `model_3d` field added to ComponentInfo, ready for GLB pipeline in S06

## Requirements Validated

- None newly validated (browser visual verification required for full 3D validation, deferred to UAT)

## New Requirements Surfaced

- None

## Requirements Invalidated or Re-scoped

- None

## Deviations

- `three` was already in package.json — only `@types/three` needed installation (plan assumed fresh install)
- Board substrate uses `BoxGeometry` with `translate()` instead of `ExtrudeGeometry` — simpler, identical visual result
- Debug surface ships `drawCalls` and `fps` from T02/T03 (plan suggested T03 stretch goals — delivered as standard)

## Known Limitations

- No real GLB/STEP component models — components shown as colored boxes. Real model loading deferred to S06.
- Browser visual verification not performed in CI (headless environment). Geometry logic validated by code review against 2D renderer coordinate conventions and unit tests.
- Pre-existing `cargo clippy --workspace --all-features` failures in `cypcb-parser` crate (51 warnings). Not introduced by S04.
- Via instancing shares one `CylinderGeometry` template with per-instance scale — minor visual imprecision for extreme size differences.

## Follow-ups

- GLB model loading pipeline (S06) — `model_3d` field ready, needs JLCPCB catalog fetch + STEP→GLB conversion
- Performance benchmarking at 60fps target on 100+ component boards (S08)
- E2E test for 3D toggle/orbit/layer visibility interactions (S07)

## Files Created/Modified

- `viewer/src/renderer3d.ts` — new, Three.js 3D renderer class (870+ lines) with full geometry pipeline and lifecycle
- `viewer/index.html` — added #view-3d-btn toggle button with CSS
- `viewer/src/main.ts` — 3D state management, lazy-import toggle handler, theme sync, keyboard shortcut, layer visibility forwarding
- `viewer/vite.config.ts` — `three` added to manualChunks for code-splitting
- `viewer/package.json` — `@types/three` added to devDependencies
- `viewer/src/types.ts` — `body_width_nm`, `body_height_nm`, `model_3d` fields added to ComponentInfo interface
- `crates/cypcb-render/src/snapshot.rs` — `body_width_nm`, `body_height_nm`, `model_3d` fields added to Rust ComponentInfo struct
- `crates/cypcb-render/src/lib.rs` — `build_snapshot()` populates body dims from footprint bounds with pad bbox fallback; new test

## Forward Intelligence

### What the next slice should know
- Three.js is fully lazy-loaded and isolated — the `Renderer3D` class is self-contained in `renderer3d.ts` with no side effects on the 2D pipeline. The 2D render loop simply skips frames when `is3DActive` is true.
- The `model_3d: Option<String>` field on ComponentInfo is plumbed through Rust→WASM→TS but always `None` currently. S06 just needs to populate it and add a GLB loader branch in `buildComponents()`.
- Layer visibility in 3D works through `updateLayerVisibility()` which maps `LayerVisibility` checkboxes to `THREE.Group.visible` toggles. Adding new layer types means adding new groups to the `layerGroups` Map.

### What's fragile
- Pad triangulation (`buildPadTriangles`) handles 4 shape types but uses approximations (octagons for roundrect/oblong). Complex pad shapes in future footprints may need more segments.
- Via size instancing uses first via as reference geometry with per-instance scale — works for boards with similar via sizes, could distort significantly if via sizes vary by >3x.

### Authoritative diagnostics
- `window.__renderer3d` in browser console — trustworthy real-time readout of scene state (mesh count, draw calls, FPS). If FPS drops, check `meshCount` and `drawCalls` first.
- `[3D]` prefixed console logs — all lifecycle events logged. Filter with `console.log` regex `\[3D\]` to trace the full init→update→dispose sequence.

### What assumptions changed
- Assumed Three.js would need fresh npm install — it was already a dependency, only types were missing.
- Assumed `ExtrudeGeometry` for board substrate — `BoxGeometry` with translate is simpler and sufficient.
