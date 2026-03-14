# T04: Canvas Renderer

**Slice:** S02 — **Milestone:** M001

## Description

Implement Canvas 2D rendering with viewport transformation and layer colors.

Purpose: Visualize board data on screen. This is the core rendering engine that transforms nanometer coordinates to screen pixels and draws components.

Output: Working canvas renderer that displays board outline, components, and pads with zoom/pan navigation.

## Must-Haves

- [ ] "Canvas renders board outline as rectangle"
- [ ] "Pads render with correct shapes and colors"
- [ ] "Zoom wheel changes scale around cursor"
- [ ] "Middle-click drag pans the view"

## Files

- `viewer/src/viewport.ts`
- `viewer/src/renderer.ts`
- `viewer/src/layers.ts`
