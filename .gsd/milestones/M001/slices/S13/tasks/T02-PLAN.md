# T02: Editor-Board Sync & Divider

**Slice:** S13 — **Milestone:** M001

## Description

Wire the Monaco editor into the application lifecycle: file open populates editor, editor changes update the board viewer in real-time, and the divider is draggable for layout customization.

Purpose: The editor and viewer must stay synchronized. The editor becomes the authoritative source of content - typing in the editor feeds the WASM engine for live preview. File operations (open, save, hot reload, desktop events) update the editor which cascades to the viewer.

Output: Fully integrated editor-viewer experience where editing .cypcb code produces immediate visual feedback in the board viewer.

## Must-Haves

- [ ] "Opening a .cypcb file populates both the editor and the board viewer"
- [ ] "Typing in the editor updates the board viewer in real-time (debounced)"
- [ ] "Draggable divider resizes editor and canvas proportions"
- [ ] "Editor content stays in sync with file operations (open, save, hot reload)"
- [ ] "Ctrl+E toggles editor visibility without losing editor content"

## Files

- `viewer/src/main.ts`
- `viewer/src/editor/editor-panel.ts`
- `viewer/index.html`
