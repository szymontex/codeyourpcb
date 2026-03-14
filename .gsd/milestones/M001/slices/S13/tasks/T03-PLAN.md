# T03: LSP Bridge & Diagnostics

**Slice:** S13 — **Milestone:** M001

## Description

Add LSP-like features (diagnostics, auto-completion, hover) to the Monaco editor using the existing WASM engine as the source of truth, without requiring a separate LSP server connection for web mode.

Purpose: EDIT-02 (auto-completion), EDIT-03 (inline errors), and EDIT-09 (LSP connection). Rather than connecting to the tower-lsp server over WebSocket (which would require a backend server), we bridge the existing WASM engine's diagnostics directly to Monaco's marker and provider APIs. The WASM engine already parses .cypcb and reports errors/violations - we just need to translate those to Monaco's format. For desktop (Tauri), the same approach works since the WASM engine runs in-browser regardless.

Output: Inline error highlighting, keyword auto-completion, and hover documentation in the Monaco editor.

## Must-Haves

- [ ] "Syntax errors from the WASM engine appear as red squiggly underlines in the editor"
- [ ] "DRC violations appear as warning markers in the editor"
- [ ] "Auto-completion suggests keywords, component types, and properties when typing"
- [ ] "Hover over keywords shows documentation tooltips"

## Files

- `viewer/src/editor/lsp-bridge.ts`
- `viewer/src/editor/editor-panel.ts`
- `viewer/src/main.ts`
