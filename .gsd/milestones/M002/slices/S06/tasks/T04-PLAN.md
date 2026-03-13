---
estimated_steps: 4
estimated_files: 5
---

# T04: Component Rotation UI, Board Resize Handles & Polish

**Slice:** S06 — Competition Feature Parity & UI Polish
**Milestone:** M002

## Description

Wire T03's WASM mutation APIs into interactive UI with undo support from T02. R key rotates selected component 90° CW, board outline gets drag handles for resize. Both operations push commands to the undo stack. Final polish pass on toolbar consistency, tooltips, and keyboard shortcut hints. This task closes EDIT-07 (undo/redo operations) and DESK-05 (keyboard shortcuts) for board-level mutations.

## Steps

1. Add `RotateComponentCommand` and `ResizeBoardCommand` to `undo.ts`. `RotateComponentCommand` stores refdes + delta_mdeg, undo negates delta. `ResizeBoardCommand` stores old/new width/height, undo restores old dimensions. Both call the engine mutation + refreshSnapshot callback.
2. Wire R key handler in main.ts: when `selectedRefdes != null`, push `RotateComponentCommand(refdes, 90000)` to undo stack (execute auto-calls engine). Shift+R → push with -90000. After execution, refresh snapshot and mark dirty for re-render. Log `[Rotate] <refdes> +90°`.
3. Board outline resize: in renderer.ts, draw 8 drag handles (4 edges + 4 corners) as small squares on board outline edges when not in routing mode. In interaction.ts, add hit-test for handles in mousedown. On drag, compute new width/height from mouse delta, push `ResizeBoardCommand` to undo stack. Min board size constraint (5mm × 5mm).
4. Polish pass: add tooltips to all new toolbar buttons (title attributes). Add keyboard shortcut hints in status bar text (R: rotate, Z: undo, Escape: clear selection). Ensure consistent spacing/separators in toolbar. Verify TypeScript compiles and Vite builds cleanly.

## Must-Haves

- [ ] R key rotates selected component 90° CW with undo support
- [ ] Shift+R rotates 90° CCW with undo support
- [ ] Board outline drag handles visible and functional
- [ ] ResizeBoardCommand undoable
- [ ] TypeScript compiles (`tsc --noEmit`) and Vite builds (`vite build`)

## Verification

- `cd viewer && npx tsc --noEmit` — zero TypeScript errors
- `cd viewer && npx vite build` — build succeeds
- `grep -q "RotateComponentCommand" viewer/src/undo.ts` — rotation command exists
- `grep -q "ResizeBoardCommand" viewer/src/undo.ts` — resize command exists
- `grep -q "KeyR\|key.*['\"]r['\"]" viewer/src/main.ts` — R key handler wired

## Inputs

- `viewer/src/undo.ts` — T02's UndoStack and BoardCommand interface
- `viewer/src/wasm.ts` — T03's rotate_component() and set_board_size() on PcbEngine
- `viewer/src/main.ts` — existing selectedRefdes state and keyboard handler patterns
- `viewer/src/renderer.ts` — existing drawBoardOutline() to extend with handles
- `viewer/src/interaction.ts` — existing mouse handler patterns

## Expected Output

- `viewer/src/undo.ts` — modified with RotateComponentCommand, ResizeBoardCommand
- `viewer/src/main.ts` — modified with R key handler, resize wiring
- `viewer/src/renderer.ts` — modified with drag handle rendering
- `viewer/src/interaction.ts` — modified with handle hit-testing and drag logic
- `viewer/index.html` — modified with tooltip attributes and status bar hints
