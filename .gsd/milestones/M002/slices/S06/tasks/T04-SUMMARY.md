---
id: T04
parent: S06
milestone: M002
provides:
  - RotateComponentCommand and ResizeBoardCommand in undo.ts with full undo/redo support
  - R key rotates selected component 90° CW, Shift+R 90° CCW, both undoable
  - 8 board outline resize drag handles (4 corners + 4 edges) with live preview and undoable resize
  - Keyboard shortcut hints in status bar and improved toolbar tooltips
key_files:
  - viewer/src/undo.ts
  - viewer/src/main.ts
  - viewer/src/renderer.ts
  - viewer/src/interaction.ts
  - viewer/index.html
key_decisions:
  - Resize drag uses live preview via direct engine.set_board_size() during drag, then reverts and pushes undo command on mouseup — keeps undo stack clean while giving instant visual feedback
  - Resize cancels on mouseleave (reverts to original dimensions) to prevent orphaned resize states
  - 8 handles (4 corners + 4 edges) rather than just corners — edge handles allow single-axis resize
patterns_established:
  - Board mutation undo pattern — RotateComponentCommand and ResizeBoardCommand follow same BoardCommand interface as trace commands; all future mutations should implement the same interface
  - Resize handle interaction pattern — hitTestResizeHandle() + resizeHandleCursor() exported from renderer.ts for reuse; interaction.ts tracks drag state and pushes undo on completion
observability_surfaces:
  - Console logs with [Rotate] prefix on component rotation (e.g. "[Rotate] R1 +90°")
  - Console logs with [Resize] prefix on board resize completion (e.g. "[Resize] Board → 100.0×80.0mm")
  - window.__undoStack debug surface reflects rotation/resize commands in lastCommand field
  - Status bar shows rotation/resize feedback text on action
duration: 25m
verification_result: passed
blocker_discovered: false
---

# T04: Component Rotation UI, Board Resize Handles & Polish

**Wired R key rotation and board outline drag-resize into interactive UI with undo support, plus toolbar polish and keyboard shortcut hints.**

## What Happened

Added `RotateComponentCommand` and `ResizeBoardCommand` to the undo system, both following the existing `BoardCommand` interface. Rotation calls `engine.rotate_component()` with delta millidegrees; undo negates the delta. Resize stores old/new dimensions; undo restores old.

R key handler in main.ts checks for `selectedRefdes` and pushes a 90° CW command (Shift+R for CCW). Both skip when editor/input is focused.

Board outline resize draws 8 handles (corners + edge midpoints) in renderer.ts via `drawResizeHandles()`. The interaction layer (interaction.ts) adds mousedown hit-testing for handles, mousemove for live-preview resize (direct engine call), and mouseup to finalize via undo stack. The pattern is: live-preview during drag → revert on mouseup → push undo command whose execute() re-applies the final dimensions. This keeps the undo stack clean with a single command per drag operation. Mouseleave cancels the drag and reverts.

Polish: added keyboard shortcut hints to the status bar (R: rotate, Z: undo, etc.), improved tooltip text on grid snap and undo/redo buttons.

## Verification

- `cd viewer && npx tsc --noEmit` — zero errors ✅
- `cd viewer && npx vite build` — build succeeds ✅
- `grep -q "RotateComponentCommand" viewer/src/undo.ts` — rotation command exists ✅
- `grep -q "ResizeBoardCommand" viewer/src/undo.ts` — resize command exists ✅
- `grep -q "e.key === 'r'" viewer/src/main.ts` — R key handler wired ✅
- `grep -q "shiftKey.*-90_000" viewer/src/main.ts` — Shift+R for CCW ✅
- `grep -q "drawResizeHandles" viewer/src/renderer.ts` — handles rendered ✅
- `grep -q "hitTestResizeHandle" viewer/src/interaction.ts` — handle hit-testing wired ✅
- All slice-level checks pass except pre-existing `cargo test -p cypcb-world` failure (sync::tests::test_sync_named_pin — unrelated to this task)
- Browser verification skipped (no display server in this environment)

## Diagnostics

- `window.__undoStack.lastCommand` — shows last rotation/resize command description
- Console: `[Rotate] R1 +90°` on R key press with selected component
- Console: `[Resize] Board → 100.0×80.0mm` on drag handle release
- Status bar updates with action feedback text

## Deviations

None — implemented exactly per plan.

## Known Issues

- Pre-existing `cargo test -p cypcb-world` failure in `sync::tests::test_sync_named_pin` — not from this task, not from this slice
- Browser visual verification deferred (no X server in build environment) — UI features are structurally correct per TypeScript compilation and code review

## Files Created/Modified

- `viewer/src/undo.ts` — Added RotateComponentCommand and ResizeBoardCommand classes
- `viewer/src/main.ts` — Added R/Shift+R key handler, onBoardResize callback, activeResizeHandle in render state, imported new commands
- `viewer/src/renderer.ts` — Added drawResizeHandles(), hitTestResizeHandle(), resizeHandleCursor(), activeResizeHandle in RenderState, handle rendering in main render loop
- `viewer/src/interaction.ts` — Added resize drag state tracking, mousedown handle hit-test, mousemove live preview, mouseup undo push, mouseleave cancel, hover cursor for handles
- `viewer/index.html` — Added keyboard shortcut hints in status bar, improved tooltip text on toolbar buttons
