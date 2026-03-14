# T01: Multi-Touch Pan & Touch-Action CSS

**Slice:** S06 — **Milestone:** M001

## Description

Add two-finger touchpad/touchscreen panning via Pointer Events API to the PCB viewer.

Purpose: NAV-02 requires touchpad gesture support for laptop users. NAV-01 (Ctrl+LMB) and NAV-03 (pinch-zoom) already work. This plan adds multi-touch pan detection using the Pointer Events API.

Output: Updated interaction.ts with pointer event handlers for two-finger pan, touch-action CSS on canvas.

## Must-Haves

- [ ] "Two-finger touchpad drag pans the viewport"
- [ ] "Existing middle-click pan still works"
- [ ] "Existing Ctrl+LMB pan still works"
- [ ] "Existing scroll wheel zoom still works"
- [ ] "Pinch-to-zoom on touchpad still works"
- [ ] "Left-click selection still works"

## Files

- `viewer/src/interaction.ts`
- `viewer/index.html`
