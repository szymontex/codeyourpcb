# T02: Viewer Integration & Drag-Drop

**Slice:** S07 — **Milestone:** M001

## Description

Integrate file picker with viewer to load .cypcb and .ses files

Purpose: Wire up file selection and drag-drop to the existing PcbEngine
Output: Working file picker that loads boards and routes into the viewer

## Must-Haves

- [ ] "Clicking Open button opens file picker dialog"
- [ ] "Selecting .cypcb file updates viewer with new board"
- [ ] "Selecting .ses file loads routes (if board already loaded)"
- [ ] "Dragging .cypcb file onto canvas loads it"
- [ ] "Status bar shows loaded filename"

## Files

- `viewer/src/main.ts`
