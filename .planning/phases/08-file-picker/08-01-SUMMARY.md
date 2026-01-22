---
phase: 08-file-picker
plan: 01
subsystem: viewer
tags: [file-api, drag-drop, typescript]

dependency_graph:
  requires: []
  provides: [file-picker-utilities, open-button, drag-drop-zone]
  affects: [08-02, 08-03]

tech_stack:
  added: []
  patterns:
    - promise-wrapper-filereader
    - hidden-input-trigger
    - drag-counter-pattern

key_files:
  created:
    - viewer/src/file-picker.ts
  modified:
    - viewer/index.html

decisions:
  - id: DEC-08-01-01
    title: Hidden input with button trigger
    choice: Hidden input element triggered by styled button
    reason: Consistent styling while maintaining accessibility

metrics:
  duration: 1m20s
  completed: 2026-01-22
---

# Phase 8 Plan 1: File Picker Infrastructure Summary

Client-side file picker with drag-drop support using native browser File API

## What Was Built

### file-picker.ts (102 lines)
- `readFileAsText(file: File)`: Promise wrapper around FileReader.readAsText()
- `createFilePicker(accept, onFile)`: Creates hidden input element with change handler
- `setupDropZone(element, onDrop)`: Sets up drag events with visual feedback class

### index.html Updates
- Open button with blue styling (#007bff)
- Drag-over CSS with dashed green outline (#28a745)
- Drop hint text that fades in during drag

## Key Implementation Details

**Drag Counter Pattern:**
The drag counter (increment on dragenter, decrement on dragleave) handles child element events correctly. Without this, the drag-over class flickers when cursor moves over child elements.

**Window-level Prevention:**
Added window-level dragover/drop handlers with preventDefault() to stop browser from navigating when files are dropped outside the canvas container.

**Input Reset:**
Reset `input.value = ''` after file selection to allow re-selecting the same file (without this, change event doesn't fire for same file).

## Commits

| Hash | Message |
|------|---------|
| 4c77362 | feat(08-01): add file picker utilities |
| ea4d48f | feat(08-01): add Open button and drag-over CSS |

## Deviations from Plan

None - plan executed exactly as written.

## Verification Results

| Check | Result |
|-------|--------|
| TypeScript compiles | Pass |
| file-picker.ts > 60 lines | Pass (102 lines) |
| readFileAsText exported | Pass |
| createFilePicker exported | Pass |
| setupDropZone exported | Pass |
| Open button in toolbar | Pass |
| Drag-over CSS present | Pass |

## Next Plan Readiness

08-02-PLAN.md (File Loading Integration) can proceed:
- file-picker.ts utilities ready for import
- Open button ready for click handler wiring
- Drop zone CSS ready for setupDropZone() call

## Files Changed

```
viewer/src/file-picker.ts  (created, 102 lines)
viewer/index.html          (modified, +35 lines)
```
