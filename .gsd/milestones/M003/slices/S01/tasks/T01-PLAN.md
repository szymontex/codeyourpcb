---
estimated_steps: 8
estimated_files: 6
---

# T01: Rewrite renderer with professional visuals, LOD, and per-pad net highlighting

**Slice:** S01 — Professional 2D Board Renderer
**Milestone:** M003

## Description

Core rendering upgrade. Extract `RenderConfig` as the boundary contract that S03 (routing pad highlighting) and S04 (preferences) will consume. Build a pad-to-net lookup from `NetInfo.connections` to enable per-pad net highlighting. Implement a 3–4 tier LOD system that controls text density by zoom level. Add all professional visual features: component body outlines, pad pin numbers, net labels on traces, world-space refdes, drill crosshair marks. Expose a diagnostic surface for E2E testability.

## Steps

1. **Create `render-config.ts`** — Define `RenderConfig` interface with: `layerColors` (top/bottom/silkscreen/via/drill), `fontConfig` (refdesWorldSize, padNumberMinScreenPx, netLabelMinSegmentPx), `lodThresholds` (scale breakpoints for 4 tiers: far/medium/close/detail), defaults factory `createDefaultRenderConfig()`. Define `LodTier` enum. Add `getLodTier(scale: number, config: RenderConfig): LodTier` function. Add `buildPadNetMap(nets: NetInfo[]): Map<string, string>` utility (key: `"refdes.padnum"`, value: net name).

2. **Write unit tests for RenderConfig** — In `viewer/src/__tests__/render-config.test.ts`: test `createDefaultRenderConfig()` returns valid config, test `getLodTier()` returns correct tier at different scales, test `buildPadNetMap()` with realistic NetInfo data (multi-pin, empty nets, power nets).

3. **Add component body outlines** — In `drawComponent()`, after drawing pads, draw a rectangle outline using `body_width_nm`/`body_height_nm` in silkscreen color (`#C8C800`). Apply component rotation. Guard against 0×0 body dimensions (skip outline). Only draw at LOD tier ≥ medium. Use dashed stroke (2px dash) to distinguish from copper.

4. **Add pad pin numbers** — New `drawPadNumbers()` function called in a text pass after all shapes. At LOD tier ≥ close: for each visible pad, if pad screen width > `config.fontConfig.padNumberMinScreenPx` (~15px), draw `pad.number` centered on the pad. Font size = `pad.width_nm * vp.scale * 0.5`, clamped 8–18px. Use contrasting color (white on dark pads, dark on light). Batch by font size to minimize `ctx.font` changes.

5. **Add net labels on traces + upgrade refdes** — New `drawTraceNetLabels()` function in text pass. At LOD tier ≥ close: for each trace, find longest segment. If segment screen length > `config.fontConfig.netLabelMinSegmentPx` (~80px), draw `trace.net_name` at midpoint, rotated along segment, small font (0.5mm world-space). For refdes: change from fixed `10px` to `config.fontConfig.refdesWorldSize * vp.scale`, clamped 8–24px. Draw at LOD tier ≥ medium.

6. **Add drill crosshair marks** — In drawPad, after drawing drill hole fill, at LOD tier ≥ medium: for THT pads with `drill_nm`, draw a small crosshair (+ shape) inside the drill hole using contrasting color. Line length = drill radius × 0.7.

7. **Per-pad net highlighting** — Pass `padNetMap` into `drawPad()`. On highlight: look up `"comp.refdes.pad.number"` in map. If pad's net matches `highlightedNet`, apply glow (same as trace glow pattern). If not, dim to 0.15 alpha. Remove the blanket dim code. Update `RenderState` to carry `padNetMap`.

8. **Wire into main.ts + diagnostics** — Import `createDefaultRenderConfig`, `buildPadNetMap`, `getLodTier` into `main.ts`. Build `padNetMap` when snapshot changes. Pass `renderConfig` and `padNetMap` into `render()`. Expose `window.__renderDiag = { lodTier, padNetMapSize, lastFrameMs, textElementsDrawn }` updated each frame. Measure frame time with `performance.now()` delta.

## Must-Haves

- [ ] `RenderConfig` interface in `render-config.ts` with layer colors, font config, LOD thresholds
- [ ] `buildPadNetMap()` correctly maps `"refdes.pin"` → net name from `NetInfo.connections`
- [ ] `getLodTier()` returns appropriate tier for given viewport scale
- [ ] Component body outlines visible in silkscreen color at medium+ zoom
- [ ] Pad pin numbers visible at close+ zoom, scaled to pad size
- [ ] Net labels at trace midpoints visible at close+ zoom
- [ ] Refdes text scales with zoom (world-space), clamped 8–24px
- [ ] Drill crosshair marks on THT pads at medium+ zoom
- [ ] Per-pad net highlighting (not blanket dim)
- [ ] `window.__renderDiag` diagnostic surface exposed
- [ ] Unit tests pass for RenderConfig, LOD, pad-net-map

## Verification

- `cd viewer && npx vitest run` — unit tests for render-config and pad-net-map pass
- Load `blink.cypcb` in browser, zoom to close level: pad numbers, net labels, body outlines, drill marks all visible
- Highlight a net: only pads on that net glow, others dim
- Zoom far out: only shapes visible, no text clutter
- `page.evaluate('window.__renderDiag')` returns object with lodTier, padNetMapSize > 0, textElementsDrawn > 0 at close zoom

## Inputs

- `viewer/src/renderer.ts` — current 842-line renderer, functional architecture with draw* functions
- `viewer/src/types.ts` — `BoardSnapshot` interfaces, `PadInfo` (has `number`, `drill_nm`), `NetInfo` (has `connections: PinRef[]`), `ComponentInfo` (has `body_width_nm`, `body_height_nm`)
- `viewer/src/layers.ts` — `LAYER_COLORS` with `top_silk`/`bottom_silk` defined but unused, `getPadColor()`, `getTraceColor()`, `netColor()`
- `viewer/src/viewport.ts` — `worldToScreen()`, scale is px/nm (0.0001 = 1mm = 100px)
- `viewer/src/main.ts` — builds `RenderState` at line 909, calls `render()` at line 924
- S01-RESEARCH.md — LOD threshold reasoning, font sizing math, text batching strategy, common pitfalls

## Expected Output

- `viewer/src/render-config.ts` — new file with `RenderConfig` interface, `LodTier` enum, `createDefaultRenderConfig()`, `getLodTier()`, `buildPadNetMap()`
- `viewer/src/renderer.ts` — rewritten with all professional visual features, LOD gating, text pass separation, per-pad net highlighting
- `viewer/src/layers.ts` — silkscreen color constant exported for body outlines (minor change)
- `viewer/src/main.ts` — wires RenderConfig, padNetMap, and diagnostic surface into render loop
- `viewer/src/__tests__/render-config.test.ts` — unit tests for config, LOD, pad-net-map
