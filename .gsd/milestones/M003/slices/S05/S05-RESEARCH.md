# S05: Project Manager & File Handling — Research

**Date:** 2026-03-13

## Summary

The app currently has no project management at all. It opens straight to an empty canvas with "Ready (WASM) - Open a file" — no recent files, no templates, no "New Project" on web. Desktop gets `file.new` from Tauri menus, but web has only the Open button and drag-drop. The existing file infrastructure is solid: `file-access.ts` wraps File System Access API with fallback, `file-picker.ts` handles drag-drop, `settings.ts` provides typed `getPreference/setPreference` with localStorage persistence and subscriber notification. The editor↔board sync (300ms debounced `onDidChangeModelContent` → `engine.load_source()` → `pullSnapshot()`) already works. The main work is (1) a project manager UI that shows on app start, (2) recent files list persisted to settings, (3) starter templates from bundled `.cypcb` examples, (4) Ctrl+N/New button for web, and (5) ensuring editor changes properly trigger board view update/reflow in all cases.

The editor→board sync already works correctly through `setupEditorSync()` in main.ts. The "editor changes trigger board view update/reflow" deliverable from the roadmap needs verification, not implementation. The new work is the project manager layer: a landing screen that replaces the blank canvas on first load.

## Recommendation

Build a project manager as a modal/overlay screen shown on app startup (when no file is loaded). It should contain three sections: (1) recent files list with name, date, and optional thumbnail, (2) template gallery using bundled `examples/*.cypcb` files, (3) import/new buttons. On file open (from recent, template, or Open button), the project manager hides and the normal editor+canvas layout takes over. Store recent files metadata in localStorage via the existing settings module (add a `recentFiles` array to `AppSettings`). Thumbnails can be generated from the Canvas 2D renderer by rendering to a small offscreen canvas and calling `toDataURL()` — the `render()` function already accepts any `CanvasRenderingContext2D`.

Keep it simple: no separate "project" abstraction, no directory structure management, no multi-file projects. This is a single-file tool. "Project" = one .cypcb file. The project manager is really a file manager with templates.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Settings persistence | `viewer/src/settings.ts` — `getPreference()`/`setPreference()` | Already wired to localStorage, has subscribers, typed, tested |
| File open/save | `viewer/src/file-access.ts` — `openFile()`/`saveFile()` | Handles File System Access API + fallback, returns handle for save-in-place |
| Drag-drop | `viewer/src/file-picker.ts` — `setupDropZone()` | Already prevents browser navigation, provides visual feedback |
| Unit formatting | `viewer/src/units.ts` — `formatDimension()` | All dimensions should go through this |
| Board rendering | `viewer/src/renderer.ts` — `render()` | Accepts any CanvasRenderingContext2D — usable for thumbnail generation |
| Theme CSS variables | `viewer/index.html` CSS custom properties | All new UI should use `var(--bg-elevated)`, `var(--text-primary)` etc. |

## Existing Code and Patterns

- `viewer/src/settings.ts` — Typed `AppSettings` with `getPreference(key)`/`setPreference(key, value)`/`subscribe(listener)`. Add `recentFiles` array here. Pattern: settings module is flat module-level state, single JSON blob in localStorage key `'cypcb-settings'`, partial-merge on load (new keys get defaults automatically).
- `viewer/src/main.ts` (line ~697) — App starts with `pullSnapshot()` then shows "Ready - Open a file". This is where the project manager screen should be shown instead. The `init()` function is 1800+ lines; project manager logic should be extracted to a separate module.
- `viewer/src/main.ts` (line ~958) — Open button handler: calls `openFile()`, loads source into engine, updates editor, fits board, shows diagnostics. This flow should also update recent files and dismiss project manager.
- `viewer/src/main.ts` (line ~544) — `setupEditorSync()` — 300ms debounced editor→board sync. Already works. Sets `dirty = true` for re-render. No changes needed.
- `viewer/src/main.ts` (line ~1169) — `reload()` function for hot-reload. Preserves viewport and selection. Template loading should use similar pattern but with fresh viewport.
- `viewer/index.html` (line ~609-670) — Current layout: `#toolbar` → `#prefs-overlay` → `#main-content` (editor + divider + canvas). Project manager overlay goes between toolbar and main-content, hidden once a file is loaded.
- `viewer/src/desktop.ts` — `handleNewFile()` dispatches `'desktop:new-file'` event. Web Ctrl+N should do the same thing (clear engine, show project manager).
- `examples/*.cypcb` — 14 example files. Good candidates for templates: `blink.cypcb` (100 lines, 555 timer LED blink), `simple-psu.cypcb` (61 lines, 5V PSU), `power-indicator.cypcb` (55 lines, LED indicator). These are small, complete, well-commented.
- `viewer/e2e/app-load.spec.ts` — Tests expect toolbar elements visible on load. Project manager will be visible instead (or alongside) — tests need updating.

## Constraints

- **main.ts is 1825 lines** — project manager must be extracted to its own module (`project-manager.ts`) to avoid further bloating. Main.ts should just wire events.
- **Templates must be bundled** — examples are at `examples/*.cypcb` in the repo root. In the built app (Vite), these need to be either inlined as string constants or fetched from a known URL. Using `fetch('/examples/blink.cypcb')` would work in dev but requires Vite public asset handling for production. Simplest: copy selected templates into `viewer/public/templates/` as static assets, or inline them as TypeScript string constants.
- **No multi-file project support** — the engine operates on a single source string. "Project" = one `.cypcb` file. Don't over-architect with project directories or workspace files.
- **File System Access API handle limitation** — `openFile()` returns a `FileSystemFileHandle` that can't be serialized to localStorage. Recent files can only store name + content preview (or thumbnail), not re-open handles. On Chrome, handles can be persisted via IndexedDB + `idb-keyval`, but that's complexity for little gain. Simpler: store name + last-modified timestamp + content hash for identification. User reopens via file picker.
- **Canvas thumbnails in headless** — E2E tests run in headless Chromium where WebGL may not produce consistent rendering. Thumbnail generation uses Canvas 2D (not WebGL), so this should be fine, but the render output may look slightly different in headless vs real browser.
- **Existing E2E tests assume immediate toolbar on load** — `app-load.spec.ts` checks for `#editor-toggle`, `#fit-btn`, etc. being visible. If project manager is a full-screen overlay, these tests might need the overlay dismissed first. Alternative: project manager is a panel within `#canvas-container`, not covering the toolbar.
- **Settings module: adding `recentFiles` to `AppSettings`** — the partial-merge pattern means existing localStorage data without `recentFiles` will automatically get the default (empty array). No migration needed.

## Common Pitfalls

- **Overbuilding project abstraction** — This is a single-file tool. Don't create Project classes, project directories, or project metadata files. A "project" is a `.cypcb` file. Keep it that way.
- **FileSystemFileHandle serialization** — These objects can't be stored in localStorage. Don't try. Recent files stores metadata only; reopening requires the user to use the file picker or drag-drop. Consider using IndexedDB for handle persistence in a future slice.
- **Template loading via fetch in production** — If templates are loaded via `fetch('/templates/blink.cypcb')`, they need to actually exist in the built output. Using Vite's `public/` directory handles this automatically, but the source examples are in `examples/` at repo root. Need a copy step or explicit public asset configuration.
- **Stale recent files** — User may have deleted or moved a file since it was in the recent list. Recent files should show gracefully even if the file is gone — they're just metadata (name, date, thumbnail), not guaranteed reopeners.
- **Thumbnail performance** — Generating thumbnails by re-rendering the board on an offscreen canvas is fine for individual files but could be slow if rendering all recent files on startup. Solution: generate thumbnail once when file is loaded/saved, store as data URL in the recent files entry.
- **Project manager z-index vs modals** — The Preferences modal and error panel use z-index. Project manager overlay needs to be below modals but above the canvas. Follow existing overlay pattern from `#prefs-overlay`.

## Open Risks

- **main.ts extraction complexity** — The `init()` function is monolithic (1800+ lines). Extracting project manager logic requires careful handling of shared state (engine, snapshot, viewport, dirty flag, editorInstance, etc.). The cleanest approach is to have `project-manager.ts` export a `showProjectManager()` / `hideProjectManager()` API and emit events (or call callbacks) when the user selects a file or template. Main.ts handles the actual engine/editor/viewport work.
- **Template curation** — Which examples become templates? Too many is overwhelming, too few is unhelpful. Recommendation: 3 templates (Blink LED — beginner, Power Indicator — intermediate, Simple PSU — intermediate+), plus "Blank" which creates an empty board scaffold.
- **Ctrl+N on web** — Currently no keyboard shortcut for new file on web. Adding Ctrl+N may conflict with browser's "new window" shortcut. Alternative: Ctrl+Shift+N (but that's incognito in Chrome). May need to just use the project manager button without a keyboard shortcut, or use a non-conflicting shortcut.
- **Editor→board reflow verification** — The roadmap says "editor changes trigger board view update/reflow." The `setupEditorSync()` debounced handler already does this. Need to verify it works for all mutation types (adding components, changing board size, modifying traces) and that the viewport adjusts appropriately (e.g., if board size changes, should it re-fit?). This may surface edge cases.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Monaco Editor | umbraco/umbraco-cms-backoffice-skills@umbraco-monaco-markdown-editor-action | not relevant (Umbraco-specific) |
| File System Access API | sundial-org/awesome-openclaw-skills@filesystem | not relevant (different abstraction layer) |
| localStorage | — | none found |
| Vite | — | already familiar, no skill needed |

No relevant skills found for the core technologies in this slice.

## Sources

- Existing codebase analysis (all findings from reading viewer/src/main.ts, settings.ts, file-access.ts, file-picker.ts, desktop.ts, renderer.ts, index.html)
- Roadmap S04→S05 boundary: S04 produces Settings API, unit display system. S05 consumes these for project manager preferences.
- S04 Summary forward intelligence: `getPreference(key)`/`setPreference(key, value)` are the API, `formatDimension(nm, unit)` for display, settings subscribe pattern returns unsubscribe fn.
