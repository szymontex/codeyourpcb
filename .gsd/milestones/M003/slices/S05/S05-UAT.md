# S05: Project Manager & File Handling — UAT

**Milestone:** M003
**Written:** 2026-03-13

## UAT Type

- UAT mode: mixed (artifact-driven for automated flows + live-runtime for visual/interaction checks)
- Why this mode is sufficient: E2E tests cover all programmatic flows (14 tests), but visual layout quality and interaction feel benefit from a human glance

## Preconditions

- `cd viewer && npm run dev` — dev server running on localhost:5173
- Browser cleared of `cypcb-settings` localStorage (fresh state): `localStorage.removeItem('cypcb-settings')`
- No `.cypcb` file loaded via URL parameter

## Smoke Test

Open http://localhost:5173 in a fresh browser tab → project manager overlay should be visible with 4 template cards (Blink LED, Power Indicator, Simple PSU, Blank Board) and a "No recent files" message below.

## Test Cases

### 1. Project manager visible on fresh load

1. Clear localStorage and open http://localhost:5173
2. Wait for page to fully load
3. **Expected:** Project manager overlay is visible, covering the canvas area but NOT the toolbar. Header says "CodeYourPCB" or similar. Four template cards are visible in a grid layout.

### 2. Template card loads board and dismisses PM

1. From the project manager, click the "Blink LED" template card
2. **Expected:** Project manager overlay disappears. Editor fills with the Blink LED `.cypcb` source code. 2D board renders showing components and traces. Canvas is now interactive (can pan/zoom).

### 3. Blank board template works

1. Reload page (PM shows again)
2. Click the "Blank Board" card
3. **Expected:** PM dismisses. Editor shows a minimal board declaration (~50×50mm, 2 layers). Canvas shows an empty board rectangle.

### 4. Recent files appear after loading a template

1. Click "Blink LED" template (PM dismisses, board loads)
2. Run in console: `window.__projectManager.show()`
3. **Expected:** PM re-appears. Recent files section now shows one entry with the name "Blink LED" (or similar) and a relative timestamp ("just now" or "a few seconds ago"). Thumbnail may be present as a small image.

### 5. Recent files persist across page reload

1. Load "Blink LED" template, then reload the page
2. **Expected:** PM appears with "Blink LED" in the recent files list, with timestamp showing the time since the previous load.

### 6. Recent files capped at 10

1. Load 12 different templates/boards in sequence (can repeat templates — each gets a unique timestamp)
2. Open PM via `window.__projectManager.show()`
3. **Expected:** Recent files list shows at most 10 entries. Oldest entries are dropped.

### 7. Open file button present and functional

1. On the PM overlay, locate the "Open File" button
2. Click it
3. **Expected:** Native file picker dialog opens (or File System Access API prompt). Selecting a valid `.cypcb` file loads it and dismisses PM.

### 8. __loadBoard hides PM

1. Open console, run: `window.__loadBoard('version 1\nboard { width: 30mm; height: 30mm; layers: 2; }')`
2. **Expected:** PM overlay disappears. Board loads and renders.

### 9. showProjectManager() re-shows PM after dismiss

1. Load a board (PM dismissed)
2. Run in console: `window.__projectManager.show()`
3. **Expected:** PM overlay reappears over the canvas, showing templates and any recent files.

### 10. Toolbar accessible while PM shown

1. With PM visible, check the toolbar at the top
2. **Expected:** Toolbar buttons (Select, Route, Editor toggle, 2D/3D, Preferences, theme toggle) are all visible and clickable above the PM overlay.

### 11. Editor→board reflow

1. Load a board with `width: 50mm; height: 50mm`
2. Toggle editor open (Editor button in toolbar)
3. Change `width: 50mm` to `width: 80mm` and `height: 50mm` to `height: 60mm` in the editor
4. Wait ~500ms for debounce
5. **Expected:** Board dimensions update to 80mm × 60mm. Run in console: `window.__pcbEngine.get_snapshot().board.width_nm` → should return `80000000`. Height should return `60000000`.

### 12. View dropdown renders above PM

1. With PM visible, click the "View" button in the toolbar
2. **Expected:** View dropdown menu appears above the PM overlay (not hidden behind it). Layer checkboxes, grid toggle, and other controls are visible and interactive.

## Edge Cases

### Template fetch failure

1. In dev tools Network tab, block requests to `/templates/blink.cypcb`
2. Click "Blink LED" template card
3. **Expected:** Console shows an error or warning. PM does not dismiss (no blank/broken state). App does not crash.

### Rapid template switching

1. Click "Blink LED" → immediately click PM show → click "Power Indicator" → immediately click PM show → click "Simple PSU"
2. **Expected:** Each template loads correctly. Recent files list accumulates entries. No visual glitches, no console errors.

### Empty localStorage

1. Clear all localStorage: `localStorage.clear()`
2. Reload page
3. **Expected:** PM appears with empty recent files ("No recent files" message). Templates work normally. Settings reset to defaults.

### Large recent files list rendering

1. Manually set localStorage with 10 recent file entries (with long names and thumbnail data URLs)
2. Reload page
3. **Expected:** Recent files list renders without overflow issues. Scroll works if needed. Names truncate gracefully.

## Failure Signals

- PM overlay not visible on fresh load → check if `showProjectManager()` is called in main.ts startup path
- Template click does nothing → check fetch to `/templates/*.cypcb` in Network tab, verify template files exist in dist/
- PM doesn't dismiss on file load → check if `hideProjectManager()` is wired to all file-load paths in main.ts
- Recent files not persisting → check `localStorage.getItem('cypcb-settings')` for `recentFiles` array
- Canvas not interactive after PM dismiss → check PM overlay still has `display: flex` (should be `display: none`)
- View dropdown hidden behind PM → check z-index values (PM: 150, dropdown should be ≥ 160)
- Editor→board reflow not working → check debounce timer (~500ms), verify `load_source()` called on editor content change

## Requirements Proved By This UAT

- Project manager lists recent files and can create new project from template (M003 milestone DoD item)
- Editor changes trigger board view update/reflow (S05 slice deliverable)

## Not Proven By This UAT

- Desktop-specific new-file flow (`desktop:new-file` event, Ctrl+N shortcut on Tauri) — web-only testing
- Drag-and-drop file import onto PM overlay — drag-drop targets the canvas behind PM
- Thumbnail quality/appearance — generated programmatically, visual fidelity not asserted
- JLCPCB integration (S06) — separate slice
- Full quality gate pass (S07) — separate slice

## Notes for Tester

- Recent file click shows info (name + date) but does NOT re-open the file — this is by design (browser security limitation: FileSystemFileHandle can't persist to localStorage)
- Thumbnail images may be null/missing for some entries — this is non-fatal, a placeholder or empty space is expected
- The PM uses z-index layering: PM at 150, View dropdown at 160, Preferences at 200. If any new overlay is added, verify it doesn't collide
- On first load after clearing localStorage, the settings module initializes defaults — theme may flash briefly (FART prevention uses a separate 'theme' localStorage key)
