---
estimated_steps: 5
estimated_files: 5
---

# T02: Grid Snap & Undo/Redo System

**Slice:** S06 — Competition Feature Parity & UI Polish
**Milestone:** M002

## Description

Implement two foundational UI features: grid snap for routing/placement and an undo/redo command stack for board mutations. These are TypeScript-only — no WASM/Rust changes needed. Grid snap extends the existing `computeSnappedPoint()` pattern. Undo/redo uses a command pattern wrapping existing `add_trace`/`remove_trace` calls. Both must be in place before T03/T04 add new mutations so those are undoable from day one.

## Steps

1. Add `snapToGrid(point: {x: number, y: number}, spacing: number): {x: number, y: number}` utility to `routing.ts`. Wire into `updatePreview()` — apply grid snap before angle snap when grid snap is enabled. Add `gridSnapEnabled` and `gridSpacing` fields to routing state.
2. Create `viewer/src/undo.ts`: `BoardCommand` interface with `execute()`, `undo()`, `description` string. `UndoStack` class with `push(cmd)`, `undo()`, `redo()`, `canUndo`, `canRedo`, `clear()`, max depth 100. Export `window.__undoStack` debug surface.
3. Implement `AddTraceCommand` and `RemoveTraceCommand` in undo.ts. Each captures the args needed to call/reverse the engine mutation plus a `refreshSnapshot` callback. Wire into existing trace add/remove flows in `main.ts` — mutations go through undo stack instead of direct engine calls.
4. Add toolbar UI: grid snap toggle checkbox, undo/redo buttons (disabled when stack empty). Follow existing toolbar HTML/CSS patterns in `index.html`.
5. Wire keyboard handlers in `main.ts`: Ctrl+Z → undo, Ctrl+Shift+Z and Ctrl+Y → redo. Clear undo stack on file load/hot-reload (alongside existing `selectedRefdes = null` reset).

## Must-Haves

- [ ] `snapToGrid()` function applied before angle snap in routing preview
- [ ] Grid snap toggleable via toolbar checkbox
- [ ] `UndoStack` class with push/undo/redo/clear, max depth 100
- [ ] `AddTraceCommand` and `RemoveTraceCommand` implementations
- [ ] Ctrl+Z / Ctrl+Shift+Z keyboard shortcuts wired
- [ ] Undo stack cleared on file load
- [ ] `window.__undoStack` debug surface

## Verification

- `cd viewer && npx tsc --noEmit` — zero TypeScript errors
- `grep -q "snapToGrid" viewer/src/routing.ts` — grid snap function exists
- `grep -q "UndoStack" viewer/src/undo.ts` — undo system exists
- `grep -q "AddTraceCommand" viewer/src/undo.ts` — trace commands exist
- `grep -q "Ctrl.*undo\|undo.*Ctrl\|ctrlKey.*undo\|KeyZ" viewer/src/main.ts` — keyboard handler wired

## Observability Impact

- Signals added: `[Undo]` prefixed console logs on execute/undo/redo with command description
- How a future agent inspects this: `window.__undoStack` in browser console → `{ canUndo, canRedo, depth, lastCommand }`
- Failure state exposed: undo/redo on empty stack is a no-op (no crash); logged as warning

## Inputs

- `viewer/src/routing.ts` — existing `computeSnappedPoint()` pattern to follow
- `viewer/src/main.ts` — existing trace add/remove flows, keyboard handler patterns
- `viewer/src/renderer.ts` — `RenderState` immutable-spread pattern
- `viewer/index.html` — existing toolbar HTML/CSS patterns

## Expected Output

- `viewer/src/undo.ts` — new file with BoardCommand, UndoStack, AddTraceCommand, RemoveTraceCommand
- `viewer/src/routing.ts` — modified with snapToGrid, gridSnapEnabled wiring
- `viewer/src/main.ts` — modified with undo/redo keyboard handlers, stack integration
- `viewer/src/renderer.ts` — gridSnap state if needed for visual feedback
- `viewer/index.html` — toolbar additions (grid snap toggle, undo/redo buttons)
