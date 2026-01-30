---
phase: 14-monaco-editor-integration
plan: 01
subsystem: editor
tags: [monaco, syntax-highlighting, ui, split-layout]
requires: [11-monaco-theme, 13-vite-optimization]
provides: [monaco-foundation, cypcb-tokenizer, editor-toggle, split-layout]
affects: [14-02-editor-wiring, 14-03-lsp-bridge]
tech-stack:
  added: [monaco-editor@0.55.x, vite-plugin-monaco-editor@1.1.x]
  patterns: [Monarch tokenizer, lazy loading, split layout]
key-files:
  created:
    - viewer/src/editor/cypcb-language.ts
    - viewer/src/editor/editor-panel.ts
  modified:
    - viewer/package.json
    - viewer/vite.config.ts
    - viewer/index.html
    - viewer/src/main.ts
decisions:
  - CommonJS Module Import Pattern: vite-plugin-monaco-editor is CommonJS, use fallback pattern (module.default || module) for ESM compatibility
  - Editor Starts Hidden: toggleable with Ctrl+E, better UX than always-on for narrow viewports
  - Minimal Workers Configuration: Only editorWorkerService worker enabled, no built-in language workers needed
metrics:
  duration: 313 seconds (5.2 minutes)
  completed: 2026-01-30
  commits: 2
---

# Phase 14 Plan 01: Monaco Editor Setup Summary

**One-liner:** Monaco editor with .cypcb Monarch syntax highlighting in toggleable side-by-side layout

## What Was Built

This plan establishes the Monaco editor foundation for CodeYourPCB. The editor loads lazily (no impact on initial page render), provides syntax highlighting for .cypcb files via a custom Monarch tokenizer, and displays in a split layout alongside the PCB canvas. Users toggle the editor with the toolbar button or Ctrl+E keyboard shortcut.

**Key capabilities delivered:**
- **EDIT-01:** Syntax highlighting for .cypcb DSL (keywords, properties, layers, numbers, strings, comments, pin references)
- **EDIT-04:** Line numbers
- **EDIT-05:** Code folding for brace blocks
- **EDIT-06:** Find/replace (Ctrl+H)
- **EDIT-07:** Undo/redo (Ctrl+Z/Ctrl+Y)
- **EDIT-08:** Multi-cursor editing (Alt+Click)
- **EDIT-10:** Side-by-side editor and board viewer

Monaco provides features EDIT-04 through EDIT-08 out of the box. The Monarch tokenizer implements EDIT-01. The split layout satisfies EDIT-10.

## Technical Implementation

### Monaco Integration

**Package versions:**
- `monaco-editor@0.55.x` - Core editor
- `vite-plugin-monaco-editor@1.1.x` - Vite bundling plugin

**Vite configuration:**
```typescript
monacoEditorPlugin({
  languageWorkers: ['editorWorkerService'],
  customWorkers: [],
})
```

Only the base editor worker is included. No built-in language workers (TypeScript, JSON, HTML, CSS) are needed since we only use our custom .cypcb language.

**Bundle size:**
- Monaco chunk: 3.77MB minified, 970KB gzipped
- Lazy-loaded via dynamic import(), no impact on initial page load
- Separate chunk enables browser caching across app updates

**Note on bundle size:** The Monaco chunk includes all of Monaco's built-in language definitions (even though workers are disabled). This is inherent to the monaco-editor package structure. Future optimization could use `monaco-editor-core` (base only, no languages) if bundle size becomes critical, but current 970KB gzipped is acceptable for lazy-loaded editor.

### Monarch Tokenizer

Custom language definition for .cypcb files:

**Token types:**
- `keyword` - Board structure (board, component, net, footprint, trace, zone, keepout, resistor, capacitor, ic, connector, diode, transistor, led, crystal, inductor, generic)
- `type` - Properties (size, layers, value, at, rotate, pin, width, clearance, current, from, to, via, layer, locked, bounds, stackup, description, pad, courtyard)
- `type.identifier` - Layer names (Top, Bottom, Inner1-4, all)
- `comment` - Line comments (//)
- `string` - Quoted strings
- `number` - Numeric values with optional units (mm, mil, mA, A, V, k, M, u, n, p, %)
- `variable` - Pin references (R1.1, C2.2, IC1.3)
- `delimiter` - Braces, parens, operators

**Language configuration:**
- Line comments: `//`
- Bracket pairs: `{}`
- Auto-closing: `{}` and `""`
- Folding markers: `{` start, `}` end

Colors map to Monaco theme tokens defined in Phase 11 (`monaco-theme.ts`).

### Split Layout

HTML structure:
```html
<div id="main-content" style="display: flex; flex: 1;">
  <div id="editor-container" style="width: 40%; display: none;">
    <!-- Monaco mounts here -->
  </div>
  <div id="divider" style="width: 4px; cursor: col-resize;"></div>
  <div id="canvas-container" style="flex: 1;">
    <!-- PCB canvas and overlays -->
  </div>
</div>
```

Editor starts hidden (`display: none`). Canvas fills available space with `flex: 1`. Divider provides visual separation and cursor hint for future drag-to-resize (not implemented in this plan).

### Lazy Loading

Editor initialization is deferred until first toggle:

```typescript
editorToggleBtn.addEventListener('click', async () => {
  if (!editorInstance) {
    console.log('[Editor] Initializing Monaco editor...');
    editorInstance = await initEditor(editorContainer);
  }
  toggleEditorPanel();
});
```

Dynamic imports in `editor-panel.ts`:
1. `import('monaco-editor')` - 970KB gzipped chunk
2. `import('../theme/monaco-theme')` - Theme wiring
3. `import('./cypcb-language')` - Monarch tokenizer (~2KB)

First click triggers all three imports in parallel, subsequent clicks are instant.

### Theme Integration

Phase 11 prepared `monaco-theme.ts` with `applyMonacoTheme(monaco)` function. This plan calls it after Monaco loads:

```typescript
const monaco = await import('monaco-editor');
const { applyMonacoTheme } = await import('../theme/monaco-theme');
applyMonacoTheme(monaco);
```

ThemeManager subscription ensures theme changes (light/dark/auto) apply to both canvas and editor simultaneously.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] CommonJS Module Import**
- **Found during:** Task 1, Vite dev server start
- **Issue:** `vite-plugin-monaco-editor` is CommonJS package with `exports.default`, ESM import failed with "not a function"
- **Fix:** Use fallback pattern `const plugin = module.default || module` with TypeScript ignore comment
- **Files modified:** `viewer/vite.config.ts`
- **Commit:** 0442de8

This is a common Node.js ESM/CommonJS interop pattern. The fallback ensures compatibility regardless of how the bundler resolves the module.

## Testing & Verification

**Build verification:**
- `npm run dev` - Vite dev server starts without errors
- `npx tsc --noEmit` - TypeScript compiles cleanly
- `npm run build:web` - Production build completes, Monaco in separate chunk

**Must-haves confirmed:**
- ✅ `viewer/src/editor/cypcb-language.ts` contains `tokenizer`
- ✅ `viewer/src/editor/editor-panel.ts` contains `editor.create`
- ✅ `viewer/vite.config.ts` contains `vite-plugin-monaco-editor`
- ✅ `viewer/index.html` contains `editor-container`
- ✅ `applyMonacoTheme()` call present in `editor-panel.ts`
- ✅ `cypcbLanguage` import present in `editor-panel.ts`

**Manual testing checklist** (for user/continuation agent):
1. Open viewer in browser
2. Click "Editor" button or press Ctrl+E → editor panel appears on left
3. Type `.cypcb` code → keywords highlighted blue, numbers green, comments olive, strings red
4. Verify line numbers visible (EDIT-04)
5. Type `board { }` → braces fold with collapse icon (EDIT-05)
6. Press Ctrl+H → find/replace dialog opens (EDIT-06)
7. Type text, press Ctrl+Z → undo works (EDIT-07)
8. Alt+Click multiple lines → multi-cursor active (EDIT-08)
9. Toggle theme (light/dark) → both canvas and editor update (Phase 11 integration)
10. Press Ctrl+E → editor hides, canvas fills space

## Next Steps

**Plan 14-02: Editor-Board Sync** will:
- Load file content into editor when opened
- Sync editor changes to engine on typing (debounced)
- Enable save from editor content
- Implement draggable divider for resizable split

**Plan 14-03: LSP Bridge** will:
- Connect to cypcb-lsp server (Phase 7)
- Register Monaco providers for completion, hover, diagnostics, goto-definition
- Handle LSP position off-by-one (Monaco 1-based, LSP 0-based)

**Architecture for Plans 14-02 and 14-03:**

The editor needs to be the single source of truth for file content. Current flow:
1. File opened → content loaded into engine
2. Engine snapshot → canvas renders

New flow (14-02):
1. File opened → content loaded into editor model
2. Editor model change (debounced 300ms) → feed to both engine and LSP
3. Engine snapshot → canvas renders
4. Save operation → read from editor model

This ensures editor, engine, and LSP stay synchronized. The editor model becomes authoritative, engine and LSP are consumers.

LSP transport (14-03):
- **Desktop mode:** Spawn cypcb-lsp as Tauri sidecar, stdio piped through IPC
- **Web mode (deferred):** Either run LSP logic in-browser via WASM (if tower-lsp compiles to WASM), or use WebSocket to backend server (when available)

For 14-03, desktop mode is sufficient. Web mode LSP can be deferred until backend infrastructure exists.

## Dependencies & Impact

**This plan depends on:**
- **Phase 11 (Theme System):** `monaco-theme.ts` with `applyMonacoTheme()` and ThemeManager
- **Phase 13 (Vite Optimization):** Vite plugin ecosystem and rollup configuration patterns

**This plan unblocks:**
- **Plan 14-02:** Editor-board sync requires Monaco instance and split layout from this plan
- **Plan 14-03:** LSP bridge requires Monaco language registration and provider APIs from this plan

**Future plans may need:**
- `editorInstance` reference to get/set content (14-02)
- `monaco.languages.register*Provider()` APIs (14-03)
- Editor resize events for draggable divider (14-02)

## Commits

| Commit | Description | Files |
|--------|-------------|-------|
| 0442de8 | Install Monaco and configure Vite plugin | package.json, vite.config.ts |
| 9d7ff13 | Create Monaco editor with split layout and .cypcb syntax highlighting | cypcb-language.ts, editor-panel.ts, index.html, main.ts |

---

**Status:** Complete
**Date:** 2026-01-30
**Duration:** 313 seconds (5.2 minutes)
**Commits:** 2
