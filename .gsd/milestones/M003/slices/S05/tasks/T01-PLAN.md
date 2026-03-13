---
estimated_steps: 7
estimated_files: 7
---

# T01: Build project manager module, HTML overlay, templates, and recent files

**Slice:** S05 — Project Manager & File Handling
**Milestone:** M003

## Description

Build the entire project manager feature: a new `project-manager.ts` module with show/hide/recent-files/thumbnail API, HTML overlay in `index.html`, bundled template files in `public/templates/`, `recentFiles` field in settings, and all wiring in `main.ts` to show on startup, hide on file load, re-show on new-file, and track recent files.

## Steps

1. **Add `recentFiles` to settings** — Define `RecentFileEntry` type (name: string, timestamp: number, thumbnail: string | null) and add `recentFiles: RecentFileEntry[]` to `AppSettings` with `[]` default. The partial-merge pattern handles migration automatically.

2. **Copy template files** — Copy `examples/blink.cypcb`, `examples/power-indicator.cypcb`, `examples/simple-psu.cypcb` into `viewer/public/templates/`. These become static assets served by Vite in both dev and prod.

3. **Create `project-manager.ts`** — Export functions:
   - `initProjectManager(callbacks)` — wires DOM event handlers, takes callbacks for `onOpenFile`, `onLoadTemplate`, `onNewBlank`
   - `showProjectManager()` — shows overlay, populates recent files from `getPreference('recentFiles')`, renders template cards
   - `hideProjectManager()` — hides overlay
   - `addRecentFile(name, snapshot?, renderState?)` — adds entry to recent files list, generates thumbnail via offscreen canvas if snapshot provided, calls `setPreference('recentFiles', ...)`, caps list at 10 entries
   - `generateThumbnail(snapshot, renderState)` — renders board to 200×150 offscreen canvas using `render()`, returns data URL
   - Debug surface: `window.__projectManager` with `{ visible, recentFiles, templateCount }`

4. **Add HTML overlay to `index.html`** — Insert `#project-manager` div between toolbar and main-content. Structure: header ("CodeYourPCB" + subtitle), templates section (3 template cards + blank card), recent files section (list populated by JS), open file button. z-index: 150 (above canvas at 0, below prefs-overlay at 200). Use existing CSS variables (`var(--bg-elevated)`, `var(--text-primary)`, etc.). Add `.hidden` class for toggle. Style template cards with grid layout, hover effect.

5. **Wire into `main.ts`** — Import `project-manager.ts`. In `init()`:
   - After WASM init but before render loop, call `initProjectManager()` with callbacks that handle engine.load_source, editor update, pullSnapshot, fitBoard, hideProjectManager
   - Show project manager on startup if no file loaded (`showProjectManager()` instead of just status text)
   - In open-file handler: after successful load, call `hideProjectManager()` + `addRecentFile(name, snapshot, renderState)`
   - In drag-drop handler: after successful load, call `hideProjectManager()` + `addRecentFile(...)`
   - In `desktop:new-file` handler: also call `showProjectManager()`
   - Add web new-file: dispatch `desktop:new-file` custom event from project manager's "New" button (reuse existing handler)
   - Template load callback: `fetch('/templates/${name}.cypcb')` → engine.load_source → editor.setValue → pullSnapshot → fitBoard → addRecentFile → hideProjectManager

6. **Blank template scaffold** — Define a minimal `.cypcb` source string in project-manager.ts for the "Blank" template (board declaration with default 50×50mm, no components). No file needed — just inline the ~10-line scaffold.

7. **Expose debug surface and verify dev build** — Ensure `window.__projectManager` updates on show/hide/addRecentFile. Run `npm run build` to confirm Vite bundles templates from public/. Manual check in dev server.

## Must-Haves

- [ ] `RecentFileEntry` type and `recentFiles` field added to `AppSettings` with `[]` default
- [ ] 3 template files in `viewer/public/templates/`
- [ ] `project-manager.ts` module with show/hide/addRecentFile/generateThumbnail exports
- [ ] HTML overlay with template cards, recent files section, open button
- [ ] Overlay shown on startup, hidden on file load, re-shown on new-file
- [ ] Recent files updated on every file open (template, open button, drag-drop)
- [ ] Recent files list capped at 10, sorted by most recent first
- [ ] Thumbnail generated from offscreen canvas render
- [ ] `window.__projectManager` debug surface exposed
- [ ] `npm run build` succeeds with no type errors

## Verification

- `npx tsc --noEmit` — zero type errors
- `npm run build` — Vite build succeeds, templates in output
- Dev server: project manager visible on fresh load, template click loads board, open button works
- After loading a file: project manager dismissed, recent files updated in localStorage

## Inputs

- `viewer/src/settings.ts` — existing settings API with getPreference/setPreference/subscribe
- `viewer/src/renderer.ts` — `render()` accepting any CanvasRenderingContext2D for thumbnail generation
- `viewer/src/file-access.ts` — `openFile()` for file picker
- `viewer/index.html` — existing overlay patterns (prefs-overlay z-index 200, view-dropdown z-index 50)
- `viewer/src/main.ts` — existing init(), open-file handler, drag-drop handler, desktop:new-file handler
- `examples/blink.cypcb`, `examples/power-indicator.cypcb`, `examples/simple-psu.cypcb` — source templates

## Expected Output

- `viewer/src/project-manager.ts` — NEW: complete project manager module
- `viewer/src/settings.ts` — MODIFIED: `RecentFileEntry` type, `recentFiles` in AppSettings
- `viewer/index.html` — MODIFIED: project manager overlay HTML + CSS
- `viewer/src/main.ts` — MODIFIED: project manager wiring (~30-50 lines added, potentially some extraction)
- `viewer/public/templates/blink.cypcb` — NEW: copied from examples
- `viewer/public/templates/power-indicator.cypcb` — NEW: copied from examples
- `viewer/public/templates/simple-psu.cypcb` — NEW: copied from examples
