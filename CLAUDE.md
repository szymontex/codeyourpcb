# CLAUDE.md — Agent Instructions for CodeYourPCB

## Keyboard Shortcuts Registry

**Every agent that adds, modifies, or removes a keyboard shortcut MUST update this file.**

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
| `Ctrl+E` | Toggle code editor panel | Global | `main.ts` |
| `3` | Toggle 3D view | Global (not in editor/input) | `main.ts` |
| `Ctrl+Shift+T` | Toggle theme (Light/Dark) | Global | `main.ts` |
| `Ctrl+S` | Save file (web only) | Global | `main.ts` |
| `Ctrl+J` | Toggle JLCPCB parts search panel | Global | `main.ts` |
| `?` | Show keyboard shortcuts help | Global (not in editor/input) | `main.ts` |
| `Escape` | Close panel / clear selection | Contextual | `main.ts` |

### Selection & Editing

| Shortcut | Action | Context | Source |
|---|---|---|---|
| `R` | Rotate selected component 90° CW | Idle mode, component selected | `main.ts` |
| `Shift+R` | Rotate selected component 90° CCW | Idle mode, component selected | `main.ts` |
| `Delete` / `Backspace` | Delete selected trace | Idle mode, trace selected | `main.ts` |
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
- `viewer/src/main.ts` — global keyboard shortcuts (Ctrl+Z, Ctrl+E, etc.) + help modal logic
- `viewer/index.html` — toolbar HTML, help modal HTML + CSS
- `CLAUDE.md` — this file (canonical shortcut registry)
