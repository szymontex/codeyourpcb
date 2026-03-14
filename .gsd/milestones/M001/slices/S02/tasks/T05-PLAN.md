# T05: Interaction Controls

**Slice:** S02 — **Milestone:** M001

## Description

Integrate rendering with interaction handling for a complete minimal viewer.

Purpose: Wire together WASM, rendering, and user interaction so the user can view and navigate their board design. This completes the core verification UI.

Output: Working PCB viewer with zoom/pan navigation, layer toggles, and component selection.

## Must-Haves

- [ ] "Board renders on screen with visible pads"
- [ ] "Scroll wheel zooms centered on cursor"
- [ ] "Middle-click drag pans the view"
- [ ] "Left-click selects component under cursor"
- [ ] "Layer checkboxes toggle visibility"

## Files

- `viewer/src/main.ts`
- `viewer/src/interaction.ts`
