# S13: Monaco Editor Integration

**Goal:** Set up Monaco editor with .
**Demo:** Set up Monaco editor with .

## Must-Haves


## Tasks

- [x] **T01: Monaco Setup & Syntax Highlighting**
  - Set up Monaco editor with .cypcb syntax highlighting and split layout alongside the PCB board viewer.

Purpose: This is the foundation for the embedded code editor. Monaco provides EDIT-04 through EDIT-08 for free (line numbers, code folding, find/replace, undo/redo, multi-cursor). The Monarch tokenizer provides EDIT-01 (syntax highlighting). The split layout provides EDIT-10 (side-by-side).

Output: Working Monaco editor panel with .cypcb syntax highlighting, toggleable via Ctrl+E, displayed alongside the existing PCB canvas viewer.
- [x] **T02: Editor-Board Sync & Divider**
  - Wire the Monaco editor into the application lifecycle: file open populates editor, editor changes update the board viewer in real-time, and the divider is draggable for layout customization.

Purpose: The editor and viewer must stay synchronized. The editor becomes the authoritative source of content - typing in the editor feeds the WASM engine for live preview. File operations (open, save, hot reload, desktop events) update the editor which cascades to the viewer.

Output: Fully integrated editor-viewer experience where editing .cypcb code produces immediate visual feedback in the board viewer.
- [x] **T03: LSP Bridge & Diagnostics**
  - Add LSP-like features (diagnostics, auto-completion, hover) to the Monaco editor using the existing WASM engine as the source of truth, without requiring a separate LSP server connection for web mode.

Purpose: EDIT-02 (auto-completion), EDIT-03 (inline errors), and EDIT-09 (LSP connection). Rather than connecting to the tower-lsp server over WebSocket (which would require a backend server), we bridge the existing WASM engine's diagnostics directly to Monaco's marker and provider APIs. The WASM engine already parses .cypcb and reports errors/violations - we just need to translate those to Monaco's format. For desktop (Tauri), the same approach works since the WASM engine runs in-browser regardless.

Output: Inline error highlighting, keyword auto-completion, and hover documentation in the Monaco editor.

## Files Likely Touched

- `viewer/package.json`
- `viewer/vite.config.ts`
- `viewer/index.html`
- `viewer/src/editor/cypcb-language.ts`
- `viewer/src/editor/editor-panel.ts`
- `viewer/src/main.ts`
- `viewer/src/editor/editor-panel.ts`
- `viewer/index.html`
- `viewer/src/editor/lsp-bridge.ts`
- `viewer/src/editor/editor-panel.ts`
- `viewer/src/main.ts`
