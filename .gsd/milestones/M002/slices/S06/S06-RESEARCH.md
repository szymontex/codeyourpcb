# S06: Competition Feature Parity & UI Polish — Research

**Date:** 2026-03-13

## Summary

S06 has two halves: (1) a deep competitive feature matrix cataloguing every capability of atopile/KiCad/Altium/Allegro/OrCAD/EAGLE/EasyEDA/diodeinc and mapping our parity status, and (2) implementing five missing UI features — grid snap, undo/redo, net highlighting, component rotation UI, and board outline editing. Both halves are frontend-heavy TypeScript work with some Rust WASM API additions for mutation operations.

The existing viewer codebase is well-structured but has zero undo/redo infrastructure, zero grid snap logic, zero net highlighting, and no board outline editing capability. Component rotation exists in the data model (`Rotation` in Rust, `rotation_mdeg` in snapshot) but there's no UI to change it. The grid is drawn cosmetically (1mm lines) but has no snap behavior.

The riskiest item is undo/redo — it requires a command pattern retrofitted across all mutation operations (trace add/remove, component move/rotate, future board outline edits). Grid snap and net highlighting are straightforward. Board outline editing requires extending the Rust `BoardWorld` from a simple width×height rectangle to a polygon, which ripples through export, rendering, and DRC.

## Recommendation

**Approach: Feature matrix first, then implement missing features in dependency order.**

1. **Feature matrix** — research-only task, produces `docs/competition-feature-matrix.md` cataloguing all tools
2. **Grid snap** — add snap-to-grid for routing waypoints and component placement; extend existing angle snap pattern
3. **Undo/redo** — implement command stack in TypeScript, wrapping existing engine mutation calls (`add_trace`, `remove_trace`, future `move_component`, `rotate_component`)
4. **Net highlighting** — click/hover a net name → highlight all traces/pads/ratsnest of that net; reuse existing `netColor()` and `colorByNet` infrastructure
5. **Component rotation UI** — expose `R` key to rotate selected component 90° CW; requires new WASM API `rotate_component(refdes, delta_mdeg)`
6. **Board outline editing** — defer polygon outline to S08 (low risk, high complexity). For S06, add UI to resize the simple rectangle (width/height) via drag handles.

This ordering reflects dependency: grid snap is independent, undo/redo should come before rotation/outline (so those mutations are undoable from day one), and net highlighting is standalone.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Command pattern for undo/redo | Custom stack in TS (no library needed) | Simple enough; canvas apps don't benefit from generic undo libs. ~100 lines for a command stack. Monaco editor already has its own undo. |
| Grid snap math | Existing `computeSnappedPoint()` in routing.ts | Extend pattern — add `snapToGrid(point, gridSpacing)` alongside angle snap |
| Net color assignment | `netColor()` in layers.ts | Already deterministic hash-to-hue with power/ground overrides |

## Existing Code and Patterns

- `viewer/src/routing.ts` — `computeSnappedPoint()` implements 45° angle snapping. Grid snap should follow same pattern: pure function `(point, gridSize) → snappedPoint`
- `viewer/src/layers.ts` — `netColor(netName)` assigns deterministic HSL colors per net. `colorByNet` flag in `RenderState` controls per-net coloring for traces. Net highlighting can extend this with an `activeNet: string | null` field.
- `viewer/src/interaction.ts` — All mouse handlers live here. Component selection already works via `query_point`. Rotation key handler goes here.
- `viewer/src/main.ts` — 800+ line init function orchestrating all state. Undo/redo keyboard handlers (Ctrl+Z/Ctrl+Shift+Z) go here. State is already functional-immutable style (spread operators for viewport/layers/routingState updates).
- `viewer/src/renderer.ts` — `drawTrace()` already supports `selectedTraceId` and `hoveredTraceId` with different visual treatments (glow, hover overlay). Net highlighting follows the same model — add an `activeNetName` to `RenderState`, dim non-matching traces.
- `viewer/src/wasm.ts` — `PcbEngine` interface defines the mutation API. New operations (`rotate_component`, `move_component`) must be added here and in the Rust WASM bridge.
- `crates/cypcb-render/src/lib.rs` — WASM `PcbEngine` struct. `add_trace_json()` and `remove_trace()` show the pattern for adding new mutation methods.
- `crates/cypcb-world/src/components/position.rs` — `Rotation` type with `from_degrees()`, `DEG_90`, `DEG_180`, `DEG_270` constants. Rust side already supports rotation — just needs a WASM-exposed mutation method.
- `crates/cypcb-world/src/world.rs` — `get_mut<T>(entity)` allows component mutation. `find_by_refdes(refdes)` locates entities. Rotation mutation: `world.get_mut::<Rotation>(entity).map(|mut r| r.0 = new_value)`.
- `viewer/src/renderer.ts` — `drawBoardOutline()` draws a simple rectangle from `width_nm × height_nm`. Board outline editing means making this interactive.
- `viewer/index.html` — Toolbar with buttons, checkboxes, separators. New UI controls (grid snap toggle, undo/redo buttons) follow existing HTML+CSS patterns.

## Constraints

- **WASM bridge is the bottleneck for new Rust-side mutations.** Every new mutation needs: (1) Rust method on `BoardWorld`, (2) WASM-exported method on `PcbEngine`, (3) TypeScript interface update, (4) adapter/mock implementation. This is ~4 files per operation.
- **No React/framework.** UI is vanilla HTML/CSS/TS. No state management library. State lives in closure variables in `init()`. Undo stack must be a plain class/module.
- **Board outline is currently just a `(Nm, Nm)` tuple** (width, height) on `BoardSize` component in Rust. No polygon support. Changing to polygon ripples through: export (gerber outline), DRC (board edge clearance), renderer (2D/3D), snapshot serialization.
- **Snapshot is rebuilt from scratch** on every `get_snapshot()` call. There's no incremental update — any mutation triggers full snapshot rebuild. This is fine for undo/redo (just replay mutations).
- **Monaco editor has its own undo/redo** for text changes. Our undo/redo is for viewer/board mutations only (trace, component, outline). These are separate undo stacks.
- **The `parseSource()` in wasm.ts is a JS parser** that re-parses the full source on every change. Component rotation via UI needs to either: (a) mutate the snapshot directly (not roundtrip through source), or (b) modify the source text and re-parse. Option (a) is cleaner — mutate the WASM world directly.
- **No existing component selection by refdes in the viewer** — `selectedRefdes` exists but there's no "selected component" concept beyond label highlighting. Need to add component selection state with visual feedback before rotation works.

## Feature Matrix Scope

The competition feature matrix needs to cover these tools:

| Tool | License | Our Access |
|------|---------|------------|
| atopile | Apache-2.0 | Cloned in `/workspace/competitors/atopile/` |
| KiCad | GPL-3.0 | Docs + architecture analysis in `docs/pcb-knowledge/competitors/` |
| diodeinc/pcb | MIT/Apache | Cloned in `/workspace/competitors/pcb/` |
| Altium Designer | Commercial | Manuals, tutorials, feature lists (web research) |
| Cadence Allegro | Commercial | Manuals, tutorials (web research) |
| Cadence OrCAD | Commercial | Manuals, tutorials (web research) |
| Autodesk EAGLE | Commercial | Manuals, tutorials (web research — note: discontinued) |
| EasyEDA | Freemium | Web app, feature docs (web research) |
| Flux.ai | Freemium | Web app, feature docs (web research) |

Categories to compare: DSL/schematic, PCB layout editing, autorouter, DRC, 3D viewer, export formats, library management, collaboration, platform support, pricing, extensibility.

## Missing Feature Analysis

### Grid Snap (complexity: LOW)

**What it is:** Cursor/component placement snaps to configurable grid (default 0.1mm or 1mm). Every professional PCB tool has this.

**Current state:** Grid is drawn cosmetically in `drawGrid()` with 1mm spacing. No snap behavior anywhere. Routing uses angle snap only.

**Implementation plan:**
- Add `gridSnap: { enabled: boolean; spacing: number }` to a new `GridState` or extend `RenderState`
- `snapToGrid(point, spacing) → point` utility — `Math.round(coord / spacing) * spacing`
- Wire into routing `updatePreview()` — snap the cursor before angle snap
- Wire into component placement (future) and outline editing
- Add UI toggle in toolbar (checkbox or button)
- Draw snap-point indicators (dots at grid intersections when zoomed in)

### Undo/Redo (complexity: MEDIUM)

**What it is:** Ctrl+Z / Ctrl+Shift+Z to undo/redo board mutations.

**Current state:** Zero undo infrastructure. Trace add/remove are fire-and-forget calls to engine.

**Implementation plan:**
- Command pattern: `interface BoardCommand { execute(): void; undo(): void; description: string }`
- `UndoStack` class with `push(cmd)`, `undo()`, `redo()`, `canUndo`, `canRedo`
- Wrap existing operations: `AddTraceCommand`, `RemoveTraceCommand`, `RotateComponentCommand`
- Keyboard handlers: Ctrl+Z → undo, Ctrl+Shift+Z (or Ctrl+Y) → redo
- Optional: toolbar undo/redo buttons
- Stack size limit (~100 commands) to prevent memory bloat
- Note: this is board-level undo only. Monaco editor has its own text undo.

### Net Highlighting (complexity: LOW)

**What it is:** Click a net name or trace → all copper belonging to that net glows/highlights. Essential for debugging connectivity.

**Current state:** `colorByNet` flag exists and traces already get per-net colors via `netColor()`. But there's no "active net" concept — all nets are colored equally.

**Implementation plan:**
- Add `highlightedNet: string | null` to `RenderState`
- When set, dim all traces/pads NOT in the highlighted net (alpha 0.2), brighten the highlighted net
- Trigger: click a trace → set `highlightedNet` to its `net_name`. Click empty space → clear.
- Also highlight pads belonging to the net (orange or bright overlay)
- Optional: show net name badge overlay
- Escape key clears highlight

### Component Rotation UI (complexity: MEDIUM)

**What it is:** Select a component, press R → rotate 90° CW. Standard in KiCad, EAGLE, etc.

**Current state:** `Rotation` type exists in Rust with `DEG_90`, etc. Components have `rotation_mdeg` in snapshot. But there's no WASM API to mutate rotation, and no UI to trigger it.

**Implementation plan:**
- Rust: add `rotate_component(&mut self, refdes: &str, delta_mdeg: i32) -> bool` to `BoardWorld`
- WASM: expose `rotate_component(refdes: string, delta_mdeg: bigint) -> boolean` on `PcbEngine`
- TS: add to `PcbEngine` interface, implement in adapter and mock
- UI: when a component is selected (`selectedRefdes != null`), R key → rotate 90° CW, Shift+R → 90° CCW
- Visual: component pads already render with rotation transform. After mutation, `get_snapshot()` returns updated `rotation_mdeg`.
- Undo: `RotateComponentCommand` wrapping the mutation.

### Board Outline Editing (complexity: HIGH — recommend deferral of polygon editing)

**What it is:** Edit the board shape — currently just a rectangle defined by `size` in DSL.

**Current state:** Board is a `(Nm, Nm)` rectangle. `drawBoardOutline()` draws it as a yellow rect. Export generates rectangular outline. Changing to polygon would require:
- New Rust data structure for board outline polygon
- Parser/DSL syntax for polygon outline
- Renderer changes (2D and 3D)
- Export changes (gerber outline layer)
- DRC board edge clearance against polygon
- Interactive editor with drag handles

**Recommendation for S06:** Implement drag handles on the rectangle (resize width/height from edges/corners). Defer polygon outline to S08.

## Common Pitfalls

- **Undo/redo state desync** — If undo removes a trace but the snapshot isn't refreshed, the renderer shows stale data. Always call `get_snapshot()` after every undo/redo operation.
- **Grid snap + angle snap conflict** — When routing, should the cursor snap to grid first then angle, or angle first then grid? KiCad does grid-first-then-angle. This is the correct order — grid determines discrete positions, angle snap selects which grid point.
- **Net highlight performance** — Dimming non-highlighted traces means redrawing every trace with alpha. With 500+ traces this could be slow. Optimization: track dirty flag per net, only redraw changed nets. But current renderer redraws everything every frame anyway, so this is fine for now.
- **Component rotation in source-parsed mode** — The WASM adapter parses source in JS and calls `load_snapshot()`. If we rotate a component via WASM API, the JS-cached snapshot is stale. Need to invalidate `this.cachedSnapshot = null` after rotation (same pattern as `add_trace`).
- **Undo stack across file reloads** — When user loads a new file or hot-reload fires, the undo stack should be cleared. Stale undo commands pointing to deleted entities would crash.

## Open Risks

- **Board outline polygon deferral** — If the feature matrix reveals that board outline editing is a critical parity gap, we may need to implement polygon outlines in S06 after all. Mitigation: do the matrix first, assess gap severity.
- **WASM mutation API expansion** — Adding `rotate_component` and potentially `move_component` requires Rust changes. The WASM build takes ~30s. If the Rust API design needs iteration, this could slow down the frontend work. Mitigation: design the full mutation API upfront, implement in one batch.
- **Feature matrix completeness** — Altium/Allegro/OrCAD are closed-source. Our knowledge comes from manuals and tutorials. Some features may be missed. Mitigation: use multiple sources (official docs, YouTube tutorials, user forums).

## Requirements Owned/Supported by S06

### Directly Owned
- **EDIT-07** (Undo/redo operations) — undo/redo for board viewer mutations
- **DESK-05** (Keyboard shortcuts Ctrl+Z etc.) — board-level undo/redo shortcuts

### Supported/Advanced
- **EDIT-01** (Syntax highlighting) — no change needed, already done
- **UI-01 through UI-09** (Theme/dark mode) — polish pass for new UI elements
- **EDIT-10** (Editor and board viewer side-by-side) — already works, no change

### Not directly owned but informed by feature matrix
- All competition parity gaps identified in the matrix feed into S07/S08 backlog

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Three.js / WebGL | `cloudai-x/threejs-skills@threejs-fundamentals` (2K installs) | available — relevant for 3D polish but S06 is mostly 2D |
| Three.js | `mrgoonie/claudekit-skills@threejs` (365 installs) | available |
| PCB / EDA | `l3wi/claude-eda@eda-pcb` (56 installs) | available — could help with feature matrix |
| KiCad file format | `o2scale/electronics-agent-kit@kicad-file-format` (26 installs) | available — useful for import/export parity |
| Tailwind design | installed: `tailwind-design-system` | installed — not applicable (no Tailwind in this project) |
| Frontend design | installed: `frontend-design` | installed — relevant for UI polish |

## Sources

- KiCad grid snap behavior: grid settings with configurable spacing, components snap on placement and during move (source: KiCad docs, existing analysis in `docs/pcb-knowledge/competitors/kicad-drc.md`)
- atopile feature set: constraint solver, module system, typed interfaces, physical units, LCSC auto-pick, package registry, VS Code extension (source: `/workspace/competitors/atopile-vs-us.md`)
- diodeinc/pcb crate structure: pcb-layout, pcb-ui (terminal only), pcb-kicad, pcb-mcp — no graphical viewer (source: `/workspace/competitors/pcb/`)
- Existing undo/redo in Monaco: Monaco editor handles text undo internally — board mutations need separate stack
- Board outline format: currently `(Nm, Nm)` tuple in `BoardSize`, drawn as rect in 2D, `BoxGeometry` in 3D (source: `crates/cypcb-world/src/world.rs`, `viewer/src/renderer.ts`, `viewer/src/renderer3d.ts`)
