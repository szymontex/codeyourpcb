# S06: Competition Feature Parity & UI Polish

**Goal:** Deep competitive feature matrix covering 9 EDA tools, plus five missing UI features implemented — grid snap, undo/redo, net highlighting, component rotation, and board outline resize.
**Demo:** User can toggle grid snap, undo/redo board mutations with Ctrl+Z/Ctrl+Shift+Z, click a trace to highlight its entire net, press R to rotate a selected component 90°, and resize the board outline via drag handles. Feature matrix document catalogues every tool's capabilities with our parity status.

## Must-Haves

- `docs/competition-feature-matrix.md` covers all 9 tools across DSL, layout, autorouter, DRC, 3D, export, library, collaboration, platform, pricing, extensibility categories
- Grid snap toggleable from toolbar, snaps routing waypoints and preview cursor to configurable grid spacing
- Undo/redo command stack with Ctrl+Z / Ctrl+Shift+Z (Ctrl+Y) keyboard shortcuts; AddTrace and RemoveTrace wrapped as commands
- Net highlighting — click a trace to highlight all copper on that net, dim everything else; Escape clears
- Component rotation — R key rotates selected component 90° CW via WASM API; wrapped as undoable command
- Board outline resize — drag handles on rectangle edges update board dimensions via WASM API; wrapped as undoable command
- TypeScript compiles cleanly (`npx tsc --noEmit`), Vite builds succeed

## Proof Level

- This slice proves: operational (UI features work through WASM bridge, feature matrix is comprehensive)
- Real runtime required: yes (browser verification for visual features, deferred to UAT in CI)
- Human/UAT required: yes (visual polish assessment)

## Verification

- `cd viewer && npx tsc --noEmit` — zero errors
- `cd viewer && npx vite build` — build succeeds
- `cargo check -p cypcb-render --all-features` — compiles
- `cargo test -p cypcb-render --all-features` — all pass (including new rotation/resize tests)
- `cargo test -p cypcb-world` — passes (including new mutation method tests)
- `test -f docs/competition-feature-matrix.md` — feature matrix exists
- `grep -c "highlightedNet" viewer/src/renderer.ts` — net highlighting field present
- `grep -c "UndoStack\|BoardCommand" viewer/src/undo.ts` — undo system exists
- `grep -c "snapToGrid" viewer/src/routing.ts` — grid snap implemented
- `grep -c "rotate_component" crates/cypcb-render/src/lib.rs` — WASM API exposed

## Observability / Diagnostics

- Runtime signals: console logs with `[Undo]` prefix for undo/redo operations, `[Grid]` for snap toggle, `[Net]` for highlight changes
- Inspection surfaces: `window.__undoStack` debug surface with `{ canUndo, canRedo, depth, lastCommand }` for undo state inspection
- Failure visibility: undo/redo operations log command description on execute/undo; WASM mutation failures logged with refdes/dimensions context

## Integration Closure

- Upstream surfaces consumed: S02 autorouter trace output, S03 renderer infrastructure, S04 3D renderer, S05 DSL v2 parser — all consumed as-is
- New wiring introduced: `UndoStack` wrapping `PcbEngine` mutation calls; `rotate_component()` and `set_board_size()` WASM exports bridging Rust↔TS
- What remains before milestone is truly usable end-to-end: S07 (E2E test suite), S08 (performance benchmarks and final polish)

## Tasks

- [x] **T01: Competition Feature Matrix** `est:45m`
  - Why: Identifies every parity gap vs 9 EDA tools before implementing features — may surface unknown gaps that change T02-T04 priorities
  - Files: `docs/competition-feature-matrix.md`
  - Do: Research atopile/KiCad/Altium/Allegro/OrCAD/EAGLE/EasyEDA/Flux.ai/diodeinc across 11 categories (DSL, layout, autorouter, DRC, 3D, export, library, collaboration, platform, pricing, extensibility). Use cloned repos for open-source tools, web research for commercial. Produce structured markdown table with our parity status (✅ parity, 🔶 partial, ❌ missing, 🚀 advantage).
  - Verify: `test -f docs/competition-feature-matrix.md && grep -c "atopile\|KiCad\|Altium" docs/competition-feature-matrix.md`
  - Done when: All 9 tools covered across all categories with honest parity assessment

- [x] **T02: Grid Snap & Undo/Redo System** `est:1h`
  - Why: Grid snap is table-stakes for any PCB editor. Undo/redo must exist before adding new mutations (rotation, resize) so they're undoable from day one. Both are TypeScript-only — no WASM changes needed.
  - Files: `viewer/src/undo.ts` (new), `viewer/src/routing.ts`, `viewer/src/main.ts`, `viewer/src/renderer.ts`, `viewer/index.html`
  - Do: (1) Add `snapToGrid(point, spacing)` utility in routing.ts alongside existing `computeSnappedPoint()`. Wire into `updatePreview()` — grid snap before angle snap. (2) Create `viewer/src/undo.ts` with `BoardCommand` interface and `UndoStack` class (~100 lines). (3) Wrap `add_trace` and `remove_trace` as `AddTraceCommand` / `RemoveTraceCommand`. (4) Add toolbar toggle for grid snap + undo/redo buttons. (5) Wire Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y keyboard handlers in main.ts. (6) Clear undo stack on file load. (7) Add `window.__undoStack` debug surface.
  - Verify: `cd viewer && npx tsc --noEmit` passes; grep confirms `UndoStack`, `snapToGrid`, keyboard handler presence
  - Done when: Grid snap toggleable, undo/redo works for trace add/remove with keyboard shortcuts, stack clears on file load

- [x] **T03: Net Highlighting & WASM Mutation APIs** `est:1h`
  - Why: Net highlighting is essential for debugging connectivity. WASM APIs for `rotate_component` and `set_board_size` are prerequisites for T04's UI features — batching all Rust changes minimizes build cycles.
  - Files: `viewer/src/renderer.ts`, `viewer/src/main.ts`, `viewer/src/wasm.ts`, `crates/cypcb-world/src/world.rs`, `crates/cypcb-render/src/lib.rs`
  - Do: (1) Add `highlightedNet: string | null` to `RenderState`. In `drawTrace()`, when `highlightedNet` is set, dim traces not matching the net (alpha 0.15) and brighten matching traces. Same for `drawPad()`. (2) On trace click in main.ts, set `highlightedNet` to the trace's `net_name`. Escape clears. (3) Rust: add `rotate_component(&mut self, refdes: &str, delta_mdeg: i32) -> bool` to `BoardWorld` using `find_by_refdes()` + `get_mut::<Rotation>()`. (4) Rust: add `set_board_size(&mut self, width_nm: i64, height_nm: i64) -> bool` to `BoardWorld`. (5) WASM: expose both as `PcbEngine` methods. (6) TS: update `PcbEngine` interface, `WasmPcbEngineAdapter`, and `MockPcbEngine`.
  - Verify: `cargo test -p cypcb-world` and `cargo test -p cypcb-render --all-features` pass; `cd viewer && npx tsc --noEmit` passes
  - Done when: Net highlighting renders correctly in drawTrace/drawPad, WASM APIs compile and are callable from TypeScript

- [x] **T04: Component Rotation UI, Board Resize Handles & Polish** `est:45m`
  - Why: Wires T03's WASM APIs into interactive UI. Component rotation and board resize complete the missing-feature set. Both must be undoable via T02's undo stack. Satisfies EDIT-07 and DESK-05 requirements for board-level undo/redo across all mutation types.
  - Files: `viewer/src/main.ts`, `viewer/src/interaction.ts`, `viewer/src/renderer.ts`, `viewer/src/undo.ts`, `viewer/index.html`
  - Do: (1) R key handler: when `selectedRefdes != null`, call `engine.rotate_component(refdes, 90000)`, push `RotateComponentCommand` to undo stack, refresh snapshot. Shift+R for CCW. (2) Board resize: draw drag handles on 4 edges of board outline in renderer.ts. Hit-test handles in interaction.ts. On drag, call `engine.set_board_size()`, push `ResizeBoardCommand` to undo stack. (3) Add `RotateComponentCommand` and `ResizeBoardCommand` to undo.ts. (4) Visual polish: consistent toolbar spacing, tooltip hints on new buttons, keyboard shortcut hints in status bar.
  - Verify: `cd viewer && npx tsc --noEmit && npx vite build` both pass; all new commands testable via undo/redo
  - Done when: R rotates component with undo, board edges draggable with undo, TypeScript compiles and builds clean

## Files Likely Touched

- `docs/competition-feature-matrix.md` (new)
- `viewer/src/undo.ts` (new)
- `viewer/src/routing.ts`
- `viewer/src/renderer.ts`
- `viewer/src/interaction.ts`
- `viewer/src/main.ts`
- `viewer/src/layers.ts`
- `viewer/src/wasm.ts`
- `viewer/index.html`
- `crates/cypcb-world/src/world.rs`
- `crates/cypcb-render/src/lib.rs`
