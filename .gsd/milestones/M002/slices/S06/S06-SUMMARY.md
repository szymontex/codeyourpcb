---
id: S06
parent: M002
milestone: M002
provides:
  - Comprehensive competition feature matrix covering 9 EDA tools across 11 categories with prioritized gap list
  - Grid snap utility with toolbar toggle, applied before angle snap in routing pipeline
  - Command-pattern undo/redo system (UndoStack, max depth 100) with Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y shortcuts
  - AddTraceCommand, RemoveTraceCommand, RotateComponentCommand, ResizeBoardCommand — all board mutations undoable
  - Net highlighting — click trace to highlight entire net, Escape to clear, non-matching traces dimmed to alpha 0.15
  - rotate_component() and set_board_size() WASM APIs bridging Rust→TS
  - 8 board outline resize drag handles (4 corners + 4 edges) with live preview and undo support
  - R/Shift+R keyboard shortcuts for 90° CW/CCW component rotation
  - Keyboard shortcut hints in status bar, improved toolbar tooltips
requires:
  - slice: S02
    provides: Autorouter trace output and board mutation pipeline
  - slice: S03
    provides: Renderer infrastructure, interaction system, trace editing
  - slice: S04
    provides: 3D renderer with component models
  - slice: S05
    provides: DSL v2 parser with modules, units, constraints
affects:
  - S07
  - S08
key_files:
  - docs/competition-feature-matrix.md
  - viewer/src/undo.ts
  - viewer/src/routing.ts
  - viewer/src/renderer.ts
  - viewer/src/interaction.ts
  - viewer/src/main.ts
  - viewer/src/wasm.ts
  - viewer/index.html
  - crates/cypcb-world/src/world.rs
  - crates/cypcb-render/src/lib.rs
key_decisions:
  - Board-level undo/redo is separate stack from Monaco editor text undo
  - Grid snap applied before angle snap (KiCad convention)
  - Board outline polygon editing deferred to S08 — S06 implements rectangle resize via drag handles only
  - WASM mutation APIs mutate BoardWorld directly rather than round-tripping through source parse
  - Command pattern (BoardCommand interface) for all board mutations with max depth 100
  - onTraceAdd callback decouples interaction.ts from undo.ts
  - BoardWorld mutation API returns bool for success/failure
  - Resize drag uses live-preview + revert-on-mouseup + undo-push pattern
  - 8 resize handles (4 corners + 4 edges) for single-axis and diagonal resize
  - Net highlight glow uses 2.0x width at 0.3 alpha, dimmed traces at 0.15 alpha
  - Pad dimming is global (pads don't carry net info in snapshot)
  - Library management identified as weakest competitive category — #1 adoption priority
patterns_established:
  - Command pattern for board mutations — all future mutations must implement BoardCommand interface
  - refreshSnapshot() centralizes engine→snapshot→UI sync for undo command callbacks
  - onTraceAdd callback pattern for routing→undo integration
  - Net highlighting pattern via highlightedNet field in RenderState
  - Resize handle interaction pattern — hitTestResizeHandle + resizeHandleCursor exported from renderer.ts
  - Parity status icons (✅ 🔶 ❌ 🚀) for feature comparison
observability_surfaces:
  - "window.__undoStack with { canUndo, canRedo, depth, lastCommand }"
  - "[Undo] prefixed console logs on execute/undo/redo"
  - "[Grid] prefixed console log on snap toggle"
  - "[Net] prefixed console logs on highlight/clear"
  - "[Rotate] prefixed console logs on component rotation"
  - "[Resize] prefixed console logs on board resize completion"
drill_down_paths:
  - .gsd/milestones/M002/slices/S06/tasks/T01-SUMMARY.md
  - .gsd/milestones/M002/slices/S06/tasks/T02-SUMMARY.md
  - .gsd/milestones/M002/slices/S06/tasks/T03-SUMMARY.md
  - .gsd/milestones/M002/slices/S06/tasks/T04-SUMMARY.md
duration: ~3h across 4 tasks
verification_result: passed
completed_at: 2026-03-13
---

# S06: Competition Feature Parity & UI Polish

**Delivered comprehensive 9-tool competitive feature matrix, grid snap, command-pattern undo/redo for all board mutations, net highlighting, component rotation, and board outline resize with drag handles.**

## What Happened

**T01 — Competition Feature Matrix:** Audited 9 EDA tools (atopile, KiCad, Altium, Allegro, OrCAD, EAGLE, EasyEDA, Flux.ai, diodeinc/pcb) across 11 categories using cloned repos and web research. Produced `docs/competition-feature-matrix.md` with per-category tables, parity status icons, summary heatmap, and prioritized 12-item gap list in 3 tiers. Key finding: our strongest areas are platform support, collaboration, and standalone autorouter; weakest is library management (no supplier API integration — identified as #1 adoption blocker).

**T02 — Grid Snap & Undo/Redo:** Built `viewer/src/undo.ts` with `BoardCommand` interface and `UndoStack` class (max 100 depth). Wrapped trace add/remove as undoable commands. Added `snapToGrid()` to routing.ts applied before angle snap. Wired Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y keyboard shortcuts (skip when Monaco focused). Added toolbar toggle for grid snap and undo/redo buttons with disabled state tracking. Installed `window.__undoStack` debug surface.

**T03 — Net Highlighting & WASM Mutations:** Added `highlightedNet` to RenderState — clicking a trace highlights its entire net (glow effect + dimming non-matching traces to 0.15 alpha). Escape clears. Added `rotate_component(refdes, delta_mdeg)` and `set_board_size(width_nm, height_nm)` to BoardWorld with unit tests, exposed on WASM PcbEngine, and mirrored across TS interface/adapter/mock.

**T04 — Rotation UI, Resize Handles & Polish:** Wired R key (90° CW) and Shift+R (90° CCW) for component rotation via undo stack. Drew 8 resize handles (4 corners + 4 edge midpoints) on board outline. Implemented drag interaction with live preview and undo-on-complete. Added keyboard shortcut hints to status bar.

## Verification

All 10 slice-level checks pass:

| Check | Result |
|-------|--------|
| `npx tsc --noEmit` | ✅ zero errors |
| `npx vite build` | ✅ succeeds |
| `cargo check -p cypcb-render --all-features` | ✅ compiles (1 pre-existing warning) |
| `cargo test -p cypcb-render --all-features` | ✅ 33 passed |
| `cargo test -p cypcb-world` | ✅ 135 passed, 1 pre-existing failure (test_sync_named_pin — unrelated) |
| `test -f docs/competition-feature-matrix.md` | ✅ exists |
| `grep highlightedNet renderer.ts` | ✅ 15 matches |
| `grep UndoStack\|BoardCommand undo.ts` | ✅ 14 matches |
| `grep snapToGrid routing.ts` | ✅ 2 matches |
| `grep rotate_component lib.rs` | ✅ 2 matches |

## Requirements Advanced

- EDIT-07 (undo/redo) — Full command-pattern undo/redo for board mutations: trace add/remove, component rotation, board resize
- DESK-05 (keyboard shortcuts) — Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y, R, Shift+R wired with editor-focus guard

## Requirements Validated

- None newly validated (EDIT-07 and DESK-05 were already validated; this slice advanced their implementation)

## New Requirements Surfaced

- None

## Requirements Invalidated or Re-scoped

- None

## Deviations

- Added `onTraceAdd` callback to `InteractionState` (not in original plan) — necessary to route trace creation through undo stack without coupling interaction.ts to undo.ts
- Grid snap settings preserved across `cancelRoute` and route completion (not explicitly planned) — required for correct UX

## Known Limitations

- Pad dimming is global when net is highlighted — pads don't carry net association in snapshot, so all pads dim rather than only non-matching pads
- Board outline editing is rectangle-only (resize handles) — polygon outline editing deferred to S08
- RemoveTraceCommand undo may assign a different trace ID after re-add — functionally correct but the ID changes
- Pre-existing `test_sync_named_pin` failure in cypcb-world — not from this slice
- Browser visual verification deferred (no X server in build environment) — structural correctness verified via TypeScript compilation, grep checks, and Rust unit tests

## Follow-ups

- S07: E2E test suite should exercise all new UI features (undo/redo, grid snap, net highlight, rotation, resize)
- S08: Board outline polygon editing (non-rectangle outlines)
- Future: Add pad-to-net mapping in snapshot for per-pad net highlighting
- Priority gap from feature matrix: supplier API integration (LCSC/Mouser) for library management

## Files Created/Modified

- `docs/competition-feature-matrix.md` — 9-tool competitive feature matrix with 11 categories and prioritized gap list
- `viewer/src/undo.ts` — BoardCommand interface, UndoStack, AddTrace/RemoveTrace/RotateComponent/ResizeBoardCommand
- `viewer/src/routing.ts` — snapToGrid() utility, grid snap state fields, grid-before-angle-snap in updatePreview
- `viewer/src/renderer.ts` — highlightedNet in RenderState, net dimming/glow in drawTrace/drawPad, drawResizeHandles, hitTestResizeHandle, resizeHandleCursor
- `viewer/src/main.ts` — Undo/redo keyboard shortcuts, grid snap toggle, net highlight on click/Escape, R/Shift+R rotation, resize callbacks, refreshSnapshot helper, debug surface init
- `viewer/src/interaction.ts` — onTraceAdd callback, grid snap preservation, resize drag state tracking with live preview
- `viewer/src/wasm.ts` — PcbEngine interface + WasmPcbEngineAdapter + MockPcbEngine updated with rotate_component and set_board_size
- `viewer/index.html` — Grid snap checkbox, undo/redo buttons, keyboard shortcut hints in status bar
- `crates/cypcb-world/src/world.rs` — rotate_component() and set_board_size() with unit tests
- `crates/cypcb-render/src/lib.rs` — WASM exports for rotate_component and set_board_size

## Forward Intelligence

### What the next slice should know
- The command pattern in `viewer/src/undo.ts` is the required pattern for all board mutations — S07 E2E tests should verify undo/redo for each command type
- The feature matrix at `docs/competition-feature-matrix.md` has a prioritized gap list that should inform S08 polish priorities
- Library management is our biggest competitive weakness — supplier integration should be considered for a future milestone

### What's fragile
- Net highlighting depends on trace `net_name` field in snapshot — if snapshot format changes, highlighting breaks silently (no error, just no highlight)
- Resize drag uses live engine mutations during drag — if engine.set_board_size() becomes async or slow, drag will stutter
- RemoveTraceCommand stores trace args and re-adds on undo — relies on engine.add_trace() producing equivalent results for same inputs

### Authoritative diagnostics
- `window.__undoStack` in browser console — shows undo stack state including last command description, depth, and can-undo/can-redo flags
- Console log prefixes `[Undo]`, `[Grid]`, `[Net]`, `[Rotate]`, `[Resize]` — grep-friendly for debugging specific feature areas
- `cargo test -p cypcb-world -- test_rotate_component test_set_board_size` — isolated verification of Rust mutation APIs

### What assumptions changed
- Original plan assumed WASM APIs needed for grid snap — turned out to be TypeScript-only (no WASM changes needed)
- Pad dimming was expected per-pad per-net — pads don't carry net info in snapshot, so global dimming was the pragmatic choice
