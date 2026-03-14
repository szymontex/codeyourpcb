# T05: Desktop Menu Event Wiring

**Slice:** S11 — **Milestone:** M001

## Description

Wire desktop menu events to the viewer engine by adding event listeners in main.ts.

Purpose: desktop.ts dispatches custom events (desktop:open-file, desktop:content-request, desktop:viewport, desktop:toggle-theme, desktop:new-file) but main.ts has no listeners for them. Without these listeners, native menu actions have no effect on the viewer.

Output: main.ts handles all desktop custom events, completing the end-to-end menu-to-viewer pipeline.

## Must-Haves

- [ ] "File > Open loads .cypcb content into the viewer engine"
- [ ] "File > Save retrieves current source content from the engine"
- [ ] "View > Zoom In/Out/Fit adjusts the viewport"
- [ ] "View > Toggle Theme cycles the theme"
- [ ] "File > New clears the current design"

## Files

- `viewer/src/main.ts`
