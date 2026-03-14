# T01: Monaco Setup & Syntax Highlighting

**Slice:** S13 — **Milestone:** M001

## Description

Set up Monaco editor with .cypcb syntax highlighting and split layout alongside the PCB board viewer.

Purpose: This is the foundation for the embedded code editor. Monaco provides EDIT-04 through EDIT-08 for free (line numbers, code folding, find/replace, undo/redo, multi-cursor). The Monarch tokenizer provides EDIT-01 (syntax highlighting). The split layout provides EDIT-10 (side-by-side).

Output: Working Monaco editor panel with .cypcb syntax highlighting, toggleable via Ctrl+E, displayed alongside the existing PCB canvas viewer.

## Must-Haves

- [ ] "Monaco editor loads in a left panel alongside the PCB canvas"
- [ ] ".cypcb files display syntax highlighting for keywords, numbers, strings, comments"
- [ ] "Editor shows line numbers, supports code folding, find/replace, undo/redo, multi-cursor"
- [ ] "Editor panel can be toggled with Ctrl+E keyboard shortcut"
- [ ] "Monaco lazy-loads so initial page load is not blocked"

## Files

- `viewer/package.json`
- `viewer/vite.config.ts`
- `viewer/index.html`
- `viewer/src/editor/cypcb-language.ts`
- `viewer/src/editor/editor-panel.ts`
