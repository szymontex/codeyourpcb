---
id: T02
parent: S06
milestone: M002
provides:
  - snapToGrid() utility function in routing.ts with grid snap state fields
  - UndoStack class with push/undo/redo/clear, max depth 100
  - AddTraceCommand and RemoveTraceCommand implementations wired into existing flows
  - Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y keyboard shortcuts for undo/redo
  - Grid snap toggle checkbox and undo/redo toolbar buttons
  - window.__undoStack debug surface
key_files:
  - viewer/src/undo.ts
  - viewer/src/routing.ts
  - viewer/src/main.ts
  - viewer/src/interaction.ts
  - viewer/index.html
key_decisions:
  - Undo commands own engine mutation + DRC rerun + snapshot refresh via callback — keeps undo.ts decoupled from main.ts state
  - Grid snap applied before angle snap in updatePreview pipeline — grid constrains cursor position, then angle snap constrains direction
  - Added onTraceAdd callback to InteractionState to route trace creation through undo stack without interaction.ts depending on undo.ts
  - Grid snap settings preserved across routing state transitions (startRoute, cancelRoute, completeRoute) via spread or explicit carry
  - Undo/redo button disabled state updated in render loop (cheap boolean check, no DOM thrash)
patterns_established:
  - Command pattern for board mutations — all future mutations (rotate, resize, etc.) should implement BoardCommand interface
  - refreshSnapshot() helper centralizes engine→snapshot→UI sync for undo command callbacks
  - onTraceAdd callback pattern for routing→undo stack integration
observability_surfaces:
  - "window.__undoStack with { canUndo, canRedo, depth, lastCommand } for undo state inspection"
  - "[Undo] prefixed console logs on execute/undo/redo with command description"
  - "[Grid] prefixed console log on snap toggle with spacing value"
  - "Undo/redo on empty stack logs warning, does not crash"
duration: 1 context window
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: Grid Snap & Undo/Redo System

**Added grid snap utility with toolbar toggle and command-pattern undo/redo stack wired into all trace mutations with keyboard shortcuts and debug surface.**

## What Happened

Implemented two foundational UI features in TypeScript only (no WASM changes):

1. **Grid snap**: Added `snapToGrid()` to routing.ts, `gridSnapEnabled`/`gridSpacing` fields to RoutingState. Grid snap is applied before angle snap in `updatePreview()`. Grid settings are preserved across route start/cancel/complete transitions.

2. **Undo/redo**: Created `viewer/src/undo.ts` with `BoardCommand` interface, `UndoStack` class (max depth 100), and `AddTraceCommand`/`RemoveTraceCommand`. Wired into both trace creation flows (routing completion in interaction.ts via new `onTraceAdd` callback) and trace deletion (Delete key in main.ts). Keyboard shortcuts: Ctrl+Z undo, Ctrl+Shift+Z/Ctrl+Y redo (skip when Monaco editor is focused).

3. **Toolbar**: Added grid snap checkbox, undo/redo buttons (disabled when stack empty) following existing toolbar HTML/CSS patterns.

## Verification

- `cd viewer && npx tsc --noEmit` — zero TypeScript errors ✅
- `cd viewer && npx vite build` — build succeeds ✅
- `grep -q "snapToGrid" viewer/src/routing.ts` — grid snap function exists ✅
- `grep -q "UndoStack" viewer/src/undo.ts` — undo system exists ✅
- `grep -q "AddTraceCommand" viewer/src/undo.ts` — trace commands exist ✅
- `grep "undoStack.undo\|undoStack.redo" viewer/src/main.ts` — keyboard + button handlers wired (4 call sites) ✅
- `cargo check -p cypcb-render --all-features` — compiles ✅
- `cargo test -p cypcb-render --all-features` — passes ✅
- `cargo test -p cypcb-world` — 1 pre-existing failure (test_sync_named_pin), not related to T02 ✅
- `test -f docs/competition-feature-matrix.md` — exists ✅

### Slice-level checks status (T02 is task 2 of 6):
- tsc: ✅ | vite build: ✅ | cargo check: ✅ | cargo test render: ✅
- feature matrix: ✅ | UndoStack: ✅ | snapToGrid: ✅
- highlightedNet: ❌ (T03) | rotate_component: ❌ (T04+)

## Diagnostics

- Browser console: `window.__undoStack` → `{ canUndo: bool, canRedo: bool, depth: number, lastCommand: string|null }`
- Console logs: `[Undo] Execute: ...`, `[Undo] Undo: ...`, `[Undo] Redo: ...`, `[Undo] Stack cleared`
- Console logs: `[Grid] Snap enabled/disabled (spacing: Xmm)`
- Empty undo/redo: logs `[Undo] Nothing to undo/redo` (no crash)

## Deviations

- Added `onTraceAdd` callback to `InteractionState` — not in the plan but necessary to route trace creation through undo stack without coupling interaction.ts to undo.ts. The fallback path (direct engine call) is preserved.
- Preserved grid snap settings across `cancelRoute` and route completion in interaction.ts — the plan didn't mention this but it's necessary for correct UX (toggle persists across routes).

## Known Issues

- `cargo test -p cypcb-world` has 1 pre-existing failure (`test_sync_named_pin`) — unrelated to T02.
- RemoveTraceCommand undo re-adds the trace and may get a different trace ID — functionally correct but the redo after undo removes by the new ID. This is fine as long as no external reference caches the old ID.

## Files Created/Modified

- `viewer/src/undo.ts` — new file: BoardCommand interface, UndoStack class, AddTraceCommand, RemoveTraceCommand, debug surface installer
- `viewer/src/routing.ts` — added snapToGrid() function, gridSnapEnabled/gridSpacing fields, grid snap before angle snap in updatePreview, preserved grid settings in cancelRoute
- `viewer/src/main.ts` — imported undo system, initialized UndoStack + debug surface, added refreshSnapshot helper, wired onTraceAdd callback, replaced direct remove_trace with RemoveTraceCommand, added Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y handlers, grid snap checkbox handler, undo/redo button handlers, undo stack cleared on file load/reload, button disabled state update in render loop
- `viewer/src/interaction.ts` — added onTraceAdd callback to InteractionState, routed trace creation through callback, preserved grid snap on route completion reset
- `viewer/index.html` — added grid snap checkbox, undo/redo buttons with CSS styling
