# S06: Competition Feature Parity & UI Polish — UAT

**Milestone:** M002
**Written:** 2026-03-13

## UAT Type

- UAT mode: mixed (artifact-driven for feature matrix, live-runtime for UI features)
- Why this mode is sufficient: Feature matrix is a document artifact verifiable by inspection. UI features (grid snap, undo/redo, net highlighting, rotation, resize) require browser interaction to confirm visual and behavioral correctness.

## Preconditions

- `cd viewer && npm run dev` running (Vite dev server)
- A `.cypcb` file loaded that contains at least 2 components and 2 traces on different nets
- Browser open to the viewer URL

## Smoke Test

Load a `.cypcb` file with components and traces. Press R with a component selected — it should rotate 90°. Press Ctrl+Z — it should rotate back. If both work, the core undo/rotation pipeline is intact.

## Test Cases

### 1. Feature Matrix Completeness

1. Open `docs/competition-feature-matrix.md`
2. Verify all 9 tools are covered: atopile, KiCad, Altium, Allegro, OrCAD, EAGLE, EasyEDA, Flux.ai, diodeinc/pcb
3. Verify all 11 categories present: DSL, layout, autorouter, DRC, 3D, export, library, collaboration, platform, pricing, extensibility
4. Verify parity status icons used consistently (✅ 🔶 ❌ 🚀)
5. Verify prioritized gap list exists with tiers
6. **Expected:** Comprehensive, honest assessment with clear gaps identified

### 2. Grid Snap Toggle

1. Load a `.cypcb` file with components
2. Check the "Grid Snap" checkbox in the toolbar
3. Start routing a trace — move cursor slowly
4. Observe cursor snapping to grid points
5. Uncheck "Grid Snap"
6. Resume routing — cursor should move freely
7. **Expected:** Cursor snaps to grid when enabled, moves freely when disabled. Console shows `[Grid] Snap enabled` / `[Grid] Snap disabled` with spacing value.

### 3. Undo/Redo Trace Operations

1. Load a `.cypcb` file
2. Add a trace by clicking two pads
3. Press Ctrl+Z
4. **Expected:** Trace disappears, console shows `[Undo] Undo: Add trace ...`
5. Press Ctrl+Shift+Z (or Ctrl+Y)
6. **Expected:** Trace reappears, console shows `[Undo] Redo: Add trace ...`
7. Select a trace and press Delete
8. Press Ctrl+Z
9. **Expected:** Deleted trace reappears

### 4. Net Highlighting

1. Load a `.cypcb` file with multiple nets
2. Click on a trace
3. **Expected:** All traces on the same net glow brighter; all other traces and pads dim (alpha ~0.15). Console shows `[Net] Highlighted: <netname>`
4. Press Escape
5. **Expected:** All traces return to normal rendering. Console shows `[Net] Cleared`
6. Click on empty space
7. **Expected:** Highlight also clears

### 5. Component Rotation

1. Load a `.cypcb` file, click a component to select it
2. Press R
3. **Expected:** Component rotates 90° clockwise. Console shows `[Rotate] <refdes> +90°`
4. Press Shift+R
5. **Expected:** Component rotates 90° counter-clockwise (back to original). Console shows `[Rotate] <refdes> -90°`
6. Press Ctrl+Z twice
7. **Expected:** Both rotations undone, component back at original orientation
8. Verify `window.__undoStack.lastCommand` shows rotation description

### 6. Board Outline Resize

1. Load a `.cypcb` file with a visible board outline
2. Hover over the right edge of the board outline
3. **Expected:** Cursor changes to `ew-resize` and a small square handle appears
4. Click and drag the edge handle to the right
5. **Expected:** Board outline stretches in real-time during drag
6. Release mouse button
7. **Expected:** Board stays at new size. Console shows `[Resize] Board → <W>×<H>mm`
8. Press Ctrl+Z
9. **Expected:** Board reverts to original size

### 7. Undo Stack Debug Surface

1. Open browser console
2. Type `window.__undoStack`
3. **Expected:** Object with `{ canUndo: boolean, canRedo: boolean, depth: number, lastCommand: string | null }`
4. Perform several undo/redo operations
5. Re-check `window.__undoStack` — values should update

### 8. Keyboard Shortcut Hints

1. Look at the status bar at the bottom of the viewer
2. **Expected:** Keyboard shortcut hints visible (R: rotate, Z: undo, etc.)
3. Hover over toolbar buttons (grid snap, undo, redo)
4. **Expected:** Tooltips appear with descriptions

## Edge Cases

### Undo on Empty Stack

1. Load a file (undo stack should be cleared)
2. Press Ctrl+Z immediately
3. **Expected:** Nothing happens. Console shows `[Undo] Nothing to undo`. No crash.

### Rotate Without Selection

1. Click empty space to deselect all components
2. Press R
3. **Expected:** Nothing happens. No error, no crash.

### Resize Cancel via Mouseleave

1. Start dragging a board resize handle
2. Move the cursor outside the canvas area
3. **Expected:** Resize cancels, board reverts to original dimensions

### Grid Snap Persists Across Routes

1. Enable grid snap
2. Start a route, cancel it (Escape)
3. Start a new route
4. **Expected:** Grid snap is still enabled for the new route

### File Reload Clears Undo Stack

1. Perform several operations (add trace, rotate component)
2. Reload the `.cypcb` file
3. Press Ctrl+Z
4. **Expected:** Nothing to undo — stack was cleared on file load. Console shows `[Undo] Stack cleared` on reload.

## Failure Signals

- TypeScript compilation errors (`npx tsc --noEmit` shows errors)
- Ctrl+Z does nothing after performing a mutation
- Net highlighting doesn't dim non-matching traces
- R key doesn't rotate selected component
- Board resize handles not visible on board edges
- Console errors related to `undefined` on undo/redo operations
- `window.__undoStack` returns `undefined`
- Undo/redo fires while typing in Monaco editor (should be guarded)

## Requirements Proved By This UAT

- EDIT-07 — Undo/redo operations work for all board mutations (trace add/remove, rotation, resize)
- DESK-05 — Keyboard shortcuts Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y, R, Shift+R function correctly

## Not Proven By This UAT

- Visual polish quality compared to commercial tools (subjective assessment needed)
- Performance under high undo stack depth (100+ operations)
- Feature matrix accuracy for commercial tools (requires domain expert review)
- Interaction with 3D view mode during undo/redo
- WASM build verification (TypeScript mock engine used in dev; real WASM tested via cargo)

## Notes for Tester

- The `test_sync_named_pin` failure in `cargo test -p cypcb-world` is pre-existing and unrelated to S06
- Net highlighting dims ALL pads (not just non-matching ones) because pads don't carry net info — this is a known limitation, not a bug
- Board resize only works on rectangle outlines — polygon editing is deferred to S08
- Grid snap spacing is currently fixed at a default value — configurable spacing is a future enhancement
- If running without WASM build, MockPcbEngine provides rotate/resize stubs that update the cached snapshot directly
