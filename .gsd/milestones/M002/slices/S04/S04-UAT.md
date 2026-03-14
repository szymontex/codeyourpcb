# S04: 3D Board Viewer — UAT

**Milestone:** M002
**Written:** 2026-03-13

## UAT Type

- UAT mode: live-runtime
- Why this mode is sufficient: 3D rendering is inherently visual — must verify in a real browser with WebGL context. Artifact-only verification cannot confirm geometry correctness, orbit behavior, or frame rate.

## Preconditions

- `cd viewer && npm run dev` running (Vite dev server on localhost:5173)
- A valid `.cypcb` file loaded that includes components, traces, and vias (e.g. `examples/blink.cypcb` or any multi-component board)
- Browser with WebGL support (Chrome, Firefox, Safari, Edge — all modern versions)

## Smoke Test

Load a .cypcb file, click the "3D" toolbar button → a green PCB slab appears with colored copper traces and component boxes. Mouse orbit rotates the view smoothly.

## Test Cases

### 1. 3D Toggle Lifecycle

1. Open a .cypcb file in the viewer — 2D board renders
2. Click the "3D" button in the toolbar
3. **Expected:** 2D canvas hides, 3D scene appears with green board substrate sized to match the board dimensions
4. Click "3D" again to toggle back
5. **Expected:** 3D scene disposes, 2D canvas returns with correct rendering. No visual artifacts or memory leaks (check browser dev tools Memory tab).

### 2. Board Substrate Dimensions

1. Toggle to 3D view
2. **Expected:** Green PCB slab matches the board outline dimensions from the .cypcb file. Board thickness ~1.6mm. Bottom face sits at Z=0.

### 3. Copper Trace Rendering

1. Load a file with routed traces (autorouted or manually defined)
2. Toggle to 3D view
3. **Expected:** Traces appear as colored ribbons on the board surface. Top copper is one color (typically red), bottom copper another (typically blue). Trace widths match specified values. Traces sit slightly above/below the substrate surface.

### 4. Pad Rendering

1. Observe component pad locations in 3D
2. **Expected:** Pads visible at correct component positions with correct rotation. Through-hole pads appear on both top and bottom layers. Pad shapes approximate the actual pad geometry (circles, rectangles, oblongs).

### 5. Via Rendering

1. Load a board with vias (multi-layer routing)
2. **Expected:** Vias appear as small cylinders spanning the board thickness. Drill holes visible as dark circles in substrate color. Via positions match 2D view.

### 6. Component Bodies

1. Toggle to 3D view on a board with multiple components
2. **Expected:** Each component has a colored box body at the correct position and rotation. ICs (U-prefix refdes) are dark gray; passives (R, C, L) are tan/beige. SMD components are ~1.2mm tall, THT components ~5mm tall.

### 7. Refdes Labels

1. Observe component bodies in 3D view
2. **Expected:** Each component body has a text label showing the reference designator (e.g. "U1", "R1", "C1"). Labels face the camera as sprites (rotate to stay readable from any angle).

### 8. Orbit/Zoom/Pan Controls

1. In 3D view, click-drag to orbit
2. Scroll wheel to zoom
3. Right-click-drag (or Ctrl+click-drag) to pan
4. **Expected:** Smooth motion with damping. Camera targets board center. Zoom range allows both close-up detail and full-board overview.

### 9. Layer Visibility

1. In 3D view, uncheck "Top" layer checkbox
2. **Expected:** Top copper traces, top pads, and top-only component bodies disappear. Board substrate and bottom layer remain visible.
3. Uncheck "Bottom", re-check "Top"
4. **Expected:** Bottom geometry hides, top geometry returns. Vias visible when either copper layer is visible.

### 10. Theme Sync

1. In 3D view, toggle theme from dark to light (or vice versa)
2. **Expected:** 3D scene background color updates to match the new theme. Board and geometry colors remain visible against both backgrounds.

### 11. Keyboard Shortcut

1. With focus on the viewer (not in the code editor), press `3`
2. **Expected:** 3D view toggles on. Press `3` again — toggles off.
3. Click into the Monaco editor, press `3`
4. **Expected:** Character '3' types in editor — shortcut does NOT fire when editor has focus.

### 12. Lazy Loading

1. Open browser Network tab, reload the page
2. **Expected:** No `three-*.js` or `renderer3d-*.js` chunks loaded on initial page load
3. Click 3D toggle
4. **Expected:** `renderer3d-*.js` and `three-*.js` chunks load on demand

## Edge Cases

### Empty Board

1. Create or load a .cypcb file with no components/traces (just a board declaration)
2. Toggle 3D
3. **Expected:** Green board substrate renders at declared dimensions. No errors in console. No copper geometry (which is correct).

### WebGL Context Loss

1. If testable: force WebGL context loss via browser dev tools
2. **Expected:** Console shows `[3D] WebGL context lost`. Status bar shows error message. View falls back to 2D gracefully.

### Rapid Toggle

1. Click 3D toggle on/off rapidly 5-10 times
2. **Expected:** No errors, no memory leaks, no orphaned WebGL contexts. Each toggle cleanly disposes previous state.

## Failure Signals

- Black or blank canvas after clicking 3D toggle (WebGL init failure)
- Console errors mentioning `THREE`, `WebGL`, or `renderer3d`
- Board substrate visibly wrong size relative to 2D view
- Components floating above or below the board surface
- Traces not visible or at wrong Z-height (hidden inside substrate)
- Frame rate drops below 30fps on a <50 component board
- Memory steadily increasing when toggling 3D on/off repeatedly (disposal leak)
- Theme toggle crashes or leaves stale background color

## Requirements Proved By This UAT

- LIB-03 (partial) — 3D model infrastructure exists; body dimensions render correctly. Full GLB model validation deferred to S06.

## Not Proven By This UAT

- 60fps performance on 100+ component boards (S08 performance benchmark)
- Real JLCPCB GLB model loading (S06 stretch feature)
- E2E automated test coverage of 3D interactions (S07)
- Cross-browser WebGL compatibility matrix (S07/S08)

## Notes for Tester

- Component bodies are colored boxes (not real 3D models). This is by design for S04 — realistic models come in S06.
- The `window.__renderer3d` object in browser console is your friend. Check `.fps`, `.meshCount`, `.drawCalls` for diagnostics.
- If the board appears tiny or enormous, the issue is likely nm→mm conversion. Check console for `[3D] Board updated: WxH mm` log — dimensions should be in reasonable mm range (e.g. 50×30mm, not 50000000×30000000nm).
- Pre-existing `cargo clippy` warnings in `cypcb-parser` are known and not related to S04.
