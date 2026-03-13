# S01: Professional 2D Board Renderer — UAT

**Milestone:** M003
**Written:** 2026-03-13

## UAT Type

- UAT mode: mixed (artifact-driven E2E + live-runtime visual inspection)
- Why this mode is sufficient: rendering quality is both measurable (diagnostic surface, test assertions) and visual (human must confirm it "looks like KiCad"). E2E covers mechanics, human covers aesthetics.

## Preconditions

- `cd viewer && npm run dev` running (dev server at localhost:5173)
- blink.cypcb loaded (default on startup)
- Browser with Canvas support (Chrome/Firefox/Safari)

## Smoke Test

Open app → blink.cypcb renders → zoom in → yellow component outlines and pad numbers appear. If you see colored shapes with text, the slice works.

## Test Cases

### 1. Component body outlines visible

1. Open app with blink.cypcb loaded
2. Zoom to medium level (components clearly visible)
3. **Expected:** Yellow dashed rectangles around component bodies. Outlines should follow component rotation.

### 2. Pad pin numbers appear at close zoom

1. Zoom in until a component fills ~1/4 of the screen
2. Look at the pads
3. **Expected:** White text numbers visible inside or near pads (e.g., "1", "2"). Numbers should scale with pad size. At far zoom, numbers should disappear (LOD culling).

### 3. Net labels on trace midpoints

1. Zoom in to see individual traces clearly
2. Look at the midpoint of longer traces
3. **Expected:** Net name labels with dark background pills, rotated along trace direction. Labels only on segments > 80px screen width.

### 4. World-space refdes scaling

1. Look at component reference designators (e.g., "R1", "C1", "U1")
2. Zoom in and out
3. **Expected:** Refdes text scales with zoom but stays readable. Should not be smaller than 8px or larger than 24px regardless of zoom level. Bold font, centered above component.

### 5. Drill crosshair marks on THT pads

1. If blink.cypcb has through-hole components, zoom to medium level
2. Look at THT pads (larger round pads with drill holes)
3. **Expected:** White crosshair (+) marks inside the drill hole area.

### 6. Per-pad net highlighting

1. Click on a trace to highlight its net
2. Look at the pads
3. **Expected:** Only pads belonging to the highlighted net glow (bright + ring). All other pads dim to ~15% opacity. This is different from the old behavior where ALL pads dimmed.

### 7. LOD transitions

1. Open browser console, type `window.__renderDiag`
2. Zoom far out, check `__renderDiag.lodTier` — should be "far" or "medium"
3. Zoom all the way in, check again — should be "close" or "detail"
4. **Expected:** Text density changes with zoom. Far zoom = no text clutter. Close zoom = pad numbers and net labels visible.

### 8. Layer colors match KiCad convention

1. Look at the board
2. **Expected:** Top copper = red, bottom copper = blue, silkscreen outlines = yellow (#C8C800). These match KiCad's default layer color scheme.

## Edge Cases

### Empty board (no components)

1. Create or load a .cypcb with just a board definition, no components
2. **Expected:** Board outline renders, no errors. Diagnostic surface shows `padNetMapSize: 0`.

### Very high zoom (detail tier)

1. Zoom in as far as possible on a component
2. **Expected:** All text visible, no Canvas rendering artifacts, frame time still under 32ms per `__renderDiag.lastFrameMs`.

### Rapid zoom in/out

1. Scroll wheel rapidly to zoom in and out
2. **Expected:** Smooth LOD transitions, no flickering text, no orphaned labels from previous zoom level.

## Failure Signals

- Yellow body outlines missing at medium zoom → silkscreen drawing broken
- No text visible at any zoom level → LOD system or text pass broken
- All pads dim equally when net highlighted → per-pad highlighting not working (reverted to blanket dim)
- `window.__renderDiag` is undefined → diagnostic surface not wired
- Frame time consistently > 32ms → LOD not culling enough text, performance regression
- Console errors mentioning `buildPadNetMap` or `RenderConfig` → wiring issue

## Requirements Proved By This UAT

- EDIT-10 — Editor and board viewer display side-by-side with professional-quality 2D visuals
- UI-09 — Canvas renderer uses proper layer colors matching theme conventions

## Not Proven By This UAT

- 3D rendering quality (S02)
- Routing UX (S03)
- Preferences persistence for colors/fonts (S04)
- Performance at 500+ components (tested at blink.cypcb scale, not stress-tested)

## Notes for Tester

- Copper fill zones won't render — there's no zone data in the current data model. That's expected, not a bug.
- Silkscreen outlines are rectangles, not the complex curves/text you'd see in KiCad — the snapshot only carries rectangular bounds. Acceptable for beta.
- The 1 pre-existing E2E failure in `errors.spec.ts:102` is unrelated to this slice — it's an M002 timing issue where status shows "Reloaded" instead of "Ready".
- `window.__renderDiag` is your best friend for debugging — it updates every frame.
