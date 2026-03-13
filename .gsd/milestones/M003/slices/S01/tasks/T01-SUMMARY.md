---
id: T01
parent: S01
milestone: M003
provides:
  - RenderConfig boundary contract (interface + defaults factory)
  - buildPadNetMap() utility for client-side pad-to-net lookup
  - getLodTier() 4-tier LOD system gating text density by zoom
  - Component body outlines in silkscreen color
  - Pad pin numbers at close+ zoom
  - Net labels on trace midpoints at close+ zoom
  - World-space refdes sizing (8–24px clamped)
  - Drill crosshair marks on THT pads
  - Per-pad net highlighting (replaces blanket dim)
  - window.__renderDiag diagnostic surface
key_files:
  - viewer/src/render-config.ts
  - viewer/src/renderer.ts
  - viewer/src/main.ts
  - viewer/src/__tests__/render-config.test.ts
  - viewer/e2e/renderer-quality.spec.ts
key_decisions:
  - Client-side pad-to-net join from NetInfo.connections (avoids Rust/WASM changes)
  - LOD thresholds calibrated to px/nm scale (medium=0.000035, close=0.00008, detail=0.0002)
  - Text rendered in separate pass after all shapes for z-ordering readability
  - pullSnapshot() helper in main.ts centralizes snapshot refresh + padNetMap rebuild
patterns_established:
  - RenderConfig as boundary contract consumed by future S03 (routing) and S04 (preferences)
  - LodTier enum gating draw calls — extend for future features
  - window.__renderDiag for E2E and runtime diagnostics
observability_surfaces:
  - "window.__renderDiag: { lodTier, padNetMapSize, lastFrameMs, textElementsDrawn }"
  - "Console warning on frames exceeding 16ms"
duration: 3h
verification_result: partial
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Rewrite renderer with professional visuals, LOD, and per-pad net highlighting

**Rewrote Canvas 2D renderer with 8 professional visual features, 4-tier LOD, per-pad net highlighting, and diagnostic surface.**

## What Happened

Created `render-config.ts` with `RenderConfig` interface (layer colors, font config, LOD thresholds), `LodTier` enum (Far/Medium/Close/Detail), `createDefaultRenderConfig()` factory, `getLodTier()` resolver, and `buildPadNetMap()` utility that joins `NetInfo.connections` into a `"refdes.pin" → netName` map.

Rewrote `renderer.ts` to add all professional features:
- **Component body outlines**: dashed rectangles in silkscreen `#C8C800` at LOD ≥ Medium, with rotation support, guarded against 0×0 bodies.
- **Pad pin numbers**: drawn in text pass at LOD ≥ Close when pad screen width > 15px. Font size = 50% of pad screen width, clamped 8–18px, white text.
- **Net labels on traces**: finds longest segment per trace, draws net name at midpoint rotated along segment with dark background pill, at LOD ≥ Close when segment > 80px screen.
- **Refdes upgrade**: world-space 0.8mm text that scales with zoom, clamped 8–24px, at LOD ≥ Medium. Bold font, centered above component.
- **Drill crosshair marks**: + shape inside drill hole at LOD ≥ Medium when drill radius > 2px. Contrasting semi-transparent white lines.
- **Per-pad net highlighting**: uses padNetMap lookup. Pads on highlighted net get brighten + glow ring. Others dim to 0.15 alpha. Replaces old blanket dim.
- **Text pass separation**: all text (refdes, pad numbers, net labels) drawn after shapes for z-ordering.
- **Diagnostic surface**: `window.__renderDiag` updated each frame with lodTier, padNetMapSize, lastFrameMs, textElementsDrawn. Console warning on frames > 16ms.

Updated `main.ts`: added `pullSnapshot()` helper that refreshes snapshot + rebuilds padNetMap in one call. All 13 snapshot-refresh sites use it. RenderConfig and padNetMap passed into render state.

Created `e2e/renderer-quality.spec.ts` skeleton with tests for: canvas renders, diagnostic surface exposed, component count, padNetMap populated, LOD tier changes with zoom.

## Verification

- **Unit tests**: `cd viewer && npx vitest run` — 55/55 pass (15 new render-config tests covering defaults, all LOD boundaries, padNetMap with multi-pin, empty nets, empty names, power nets, duplicate pins)
- **TypeScript**: `npx tsc --noEmit` — zero errors
- **Slice verification (vitest)**: ✅ PASS — unit tests for pad-to-net lookup, LOD threshold logic, RenderConfig defaults all pass
- **Slice verification (playwright E2E)**: ⏳ NOT RUN — no X server in current environment. Test file created at `viewer/e2e/renderer-quality.spec.ts`, to be verified in T02 or CI.
- **Slice verification (manual visual)**: ⏳ NOT RUN — requires browser with display. All rendering code is wired and TypeScript-verified.

## Diagnostics

- `window.__renderDiag` in browser console shows live LOD tier, pad-net map size, frame time, text count
- Console warning `[renderer] Slow frame: Xms` when frame > 16ms
- `buildPadNetMap()` is pure function — testable in isolation with mock NetInfo arrays

## Deviations

- E2E test created as skeleton rather than fully validated — no X server available for Playwright. Test structure follows existing patterns (app-load.spec.ts). T02 will validate E2E.
- `layers.ts` not modified — silkscreen color drawn directly from `config.layerColors.silkscreen` in renderer rather than adding a new export to layers.ts. Cleaner this way since RenderConfig owns all colors.

## Known Issues

- Drill crosshair LOD check creates a default config inline if no config passed — minor inefficiency but only affects edge case where RenderConfig is not provided (defensive fallback removed, lodTier now passed as parameter).
- E2E tests need browser environment to validate. Created but untested in this context.

## Files Created/Modified

- `viewer/src/render-config.ts` — NEW: RenderConfig interface, LodTier enum, defaults factory, getLodTier(), buildPadNetMap()
- `viewer/src/renderer.ts` — REWRITTEN: all 8 professional features, LOD gating, text pass, per-pad highlighting, diagnostic surface
- `viewer/src/main.ts` — MODIFIED: pullSnapshot() helper, renderConfig/padNetMap wiring, import additions
- `viewer/src/__tests__/render-config.test.ts` — NEW: 15 unit tests for config, LOD, pad-net-map
- `viewer/e2e/renderer-quality.spec.ts` — NEW: E2E test skeleton for renderer quality verification
