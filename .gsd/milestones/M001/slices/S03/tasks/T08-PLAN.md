# T08: Violation Display

**Slice:** S03 — **Milestone:** M001

## Description

Implement violation display in the viewer with markers and status bar.

Purpose: Visual DRC feedback per user decisions - circle markers at violation locations, VS Code-style status bar with error count, click-to-zoom functionality.

Output: Viewer renders DRC violations as markers with non-invasive error panel.

## Must-Haves

- [ ] "Violation markers visible as circles/rings on board"
- [ ] "Status bar shows error count"
- [ ] "Clicking error in list zooms to location"
- [ ] "Error panel is non-invasive (VS Code style)"

## Files

- `viewer/src/renderer.ts`
- `viewer/src/main.ts`
- `viewer/index.html`
- `viewer/src/layers.ts`
