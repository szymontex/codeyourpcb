# CLAUDE.md — Agent Instructions for CodeYourPCB

## Toolbar Architecture

The top toolbar (`viewer/index.html`, `<div id="toolbar">`) uses a flat, icon-only style.
All buttons use the `.tb-btn` class — transparent background, no borders, `color: var(--text-secondary)`.

**Key design rule:** No filled/colored buttons in normal state. Route button follows the same `.tb-btn` style as all other buttons. It only gets colored background (`--warning`) when actively routing (`.routing` class added by JS).

### Route split-button group
- `#route-btn` — text-only "Route" label, triggers autorouting (`triggerRouting()`)
- `#route-menu-btn` — dropdown caret (▾), opens routing options menu with auto-route checkbox + 4 tuning sliders
- Both wrapped in `.tb-route-group` flex container
- During active routing: both get `.routing` class → yellow background

### JLCPCB search button
- `#jlcpcb-search-btn` — magnifying glass icon in toolbar (right side, before settings gear)
- Opens `#jlcpcb-search-panel` (right-side overlay)
- Same keyboard shortcut: `Ctrl+J`
- Uses `.tb-btn` class like all other toolbar buttons

### Settings button
- `#prefs-btn` — gear icon, rightmost in toolbar
- Opens preferences modal overlay

## Project Manager

The project manager (`#project-manager`) is the startup screen and file browser. Source: `viewer/src/project-manager.ts`.

### Sections (top to bottom):
1. **Start from template** — 4 bundled templates + blank board (static cards)
2. **Workspace** (`#pm-projects-section`) — `.cypcb` files from dev server's watch directory. Only visible when WebSocket dev server (`server.ts`) is connected. Hidden by default (`display:none`).
3. **Your projects** (`#pm-recent-section`) — recent files stored in localStorage with rendered thumbnails. Hidden when empty. This is the primary project list for web-only users without dev server.
4. **Import File** (`#pm-open-btn`) — opens native file picker (File System Access API or fallback)

### Thumbnail generation
- Thumbnails are rendered via offscreen canvas (400×300px) using the same `render()` function as the main view
- Generated in `generateThumbnail()` in `project-manager.ts`
- Stored as base64 PNG data URLs in localStorage (`cypcb-settings` → `recentFiles[]`)
- **LCSC footprint timing issue:** Components with `lcsc` attributes have no pads until async EasyEDA API fetch completes. Two mechanisms ensure thumbnails show components:
  1. `reloadAfterLcscFetch()` in `main.ts` — regenerates thumbnail after async footprint fetch
  2. `onRefreshThumbnail` callback — regenerates thumbnail from current snapshot every time PM opens (catches any missed updates)
- Thumbnails use `width: 100%; aspect-ratio: 4/3` CSS for responsive sizing within grid cards

### WebSocket integration (dev server only)
- `onConnect` callback sends `{ type: 'list-files' }` to get workspace file list
- `onFileList` callback populates `#pm-project-list` grid
- `onOpenProjectFile` sends `{ type: 'open-file', file: path }` — server responds with `reload` message
- Server handler `handleOpenFileRequest()` reads file and sends content as `reload` type

### Naming conventions
- "Open" = open existing project from workspace/recent list
- "Import" = upload file from local disk (file picker)
- Display names strip `.cypcb` extension

## File Structure (shortcuts-related)

When adding a new shortcut:
1. Add it to the appropriate section below
2. Add the corresponding `<kbd>` row in the help modal HTML (`viewer/index.html`, inside `#help-modal`)
3. Keep both this file and the help modal in sync

### General

| Shortcut | Action | Context | Source |
|---|---|---|---|
| `Ctrl+Z` | Undo | Global (not in editor/input) | `main.ts` |
| `Ctrl+Shift+Z` / `Ctrl+Y` | Redo | Global (not in editor/input) | `main.ts` |
| `F` | Fit board to view | When not routing | `main.ts` |
| `L` | Next copper layer down the stack | When not routing | `main.ts` |
| `Shift+L` | Next copper layer up the stack | When not routing | `main.ts` |
| `1`-`9` | Jump straight to that copper layer | When not routing | `main.ts` |
| `X` | Cycle layer focus: all / grey others / dim others / active only | Global, routing included | `main.ts` |
| `Ctrl+Tab` | Walk the saved layer views: everything / front / back / copper only | Global | `main.ts` |
| `Ctrl+E` | Toggle code editor panel | Global | `main.ts` |
| `3` | Toggle 3D view | Global (not in editor/input) | `main.ts` |
| `Ctrl+Shift+T` | Toggle theme (Light/Dark) | Global | `main.ts` |
| `Ctrl+S` | Save file (web only) | Global | `main.ts` |
| `Ctrl+O` | Open file / project manager | Global | `main.ts` |
| `Ctrl+J` | Toggle JLCPCB parts search panel | Global | `main.ts` |
| `?` | Show keyboard shortcuts help | Global (not in editor/input) | `main.ts` |
| `Escape` | Close panel / clear selection | Contextual | `main.ts` |

### Selection & Editing

| Shortcut | Action | Context | Source |
|---|---|---|---|
| `R` | Rotate selected component 90° CW | Idle mode, component selected | `main.ts` |
| `Shift+R` | Rotate selected component 90° CCW | Idle mode, component selected | `main.ts` |
| `Delete` / `Backspace` | Delete selected trace | Idle mode, trace selected | `main.ts` |
| `Ctrl+L` | Simplify selected trace | Idle mode, trace selected | `interaction.ts` |
| Click+drag segment | Drag trace segment | Idle mode, trace segment | `interaction.ts` |
| Click+drag vertex | Drag trace corner | Idle mode, trace vertex | `interaction.ts` |
| Click+drag empty space | Rectangle select | Idle mode | `interaction.ts` |

### Trace Segment Editing (mouse)

| Input | Action | Source |
|---|---|---|
| Click+drag on trace segment | Drag segment parallel (45° constrained) | `interaction.ts` |
| Click+drag on trace corner | Drag vertex with 45° re-route | `interaction.ts` |
| Click+drag on empty space | Rectangle selection | `interaction.ts` |

### Interactive Routing (active only during manual trace routing)

| Shortcut | Action | Context | Source |
|---|---|---|---|
| `F` | Flip routing layer (top ↔ bottom) | During routing | `interaction.ts` |
| `A` | Toggle angle snap (free ↔ grid-snapped) | During routing | `interaction.ts` |
| `/` | Flip posture (straight-first ↔ diagonal-first) | During routing | `interaction.ts` |
| `Q` | Toggle corner mode (45° mitered ↔ 90° only) | During routing | `interaction.ts` |
| `Escape` | Cancel current route | During routing | `interaction.ts` |

### Navigation (Mouse)

| Input | Action | Source |
|---|---|---|
| Scroll wheel | Zoom (centered on cursor) | `interaction.ts` |
| Middle-click + drag | Pan | `interaction.ts` |
| Ctrl + left-click + drag | Pan (laptop alternative) | `interaction.ts` |
| Two-finger touchpad drag | Pan | `interaction.ts` |
| Left-click on pad | Start routing from pad | `interaction.ts` |
| Left-click on trace | Select trace | `interaction.ts` |
| Left-click on resize handle | Drag to resize board | `interaction.ts` |
| Left-click+drag on trace segment | Drag segment parallel (45° constrained) | `interaction.ts` |
| Left-click+drag on trace corner | Drag vertex with 45° re-route | `interaction.ts` |
| Left-click+drag on empty space | Rectangle selection | `interaction.ts` |

## Help Modal

The keyboard shortcuts help dialog lives in `viewer/index.html` inside `<div id="help-overlay">`.
It is opened by the `?` button in the toolbar or by pressing `?` on the keyboard.
JS handlers are in `viewer/src/main.ts` (search for `helpBtn`, `openHelpModal`, `closeHelpModal`).

When adding a shortcut, add a `<div class="help-row">` entry in the appropriate `<div class="help-section">`:

```html
<div class="help-row">
  <span class="help-desc">Description of action</span>
  <kbd>Key combo</kbd>
</div>
```

## File Structure (shortcuts-related)

- `viewer/src/interaction.ts` — mouse/touch handlers + routing keyboard shortcuts (F, A, /, Q, Escape during routing)
- `viewer/src/main.ts` — global keyboard shortcuts (Ctrl+Z, Ctrl+E, etc.) + help modal logic + WS connection + project manager callbacks
- `viewer/src/project-manager.ts` — project manager overlay (templates, recent files, workspace files, thumbnail generation)
- `viewer/src/settings.ts` — settings persistence (localStorage), RecentFileEntry type
- `viewer/server.ts` — dev server with WS (file watching, routing, list-files, open-file)
- `viewer/index.html` — toolbar HTML, help modal HTML + CSS, project manager HTML + CSS
- `CLAUDE.md` — this file (canonical shortcut registry + architecture docs)
