# S01: Professional 2D Board Renderer — Research

**Date:** 2026-03-13

## Summary

The current `renderer.ts` (842 lines) draws basic pad shapes, traces, vias, ratsnest, and a tiny refdes label at component center — but nothing approaching professional PCB quality. It's missing: pad pin numbers, net labels on traces, component body outlines (silkscreen), zoom-dependent level-of-detail, proper drill marks as distinct visual elements, and pad-level net highlighting. The data model in `BoardSnapshot` already carries almost everything needed — pad numbers, net names on traces, body dimensions, drill info, layer masks — so this is predominantly a rendering-layer rewrite, not a data pipeline problem.

The one notable data gap is pad-to-net mapping: pads in the snapshot don't carry net info (confirmed by `DECISIONS.md`), and the current renderer dims *all* pads when a net is highlighted. Fixing this requires either a Rust snapshot change (adding `net_id` to `PadInfo`) or a client-side lookup table built from `NetInfo.connections`. The client-side join is fast and avoids cross-boundary changes, so that's the recommended path for S01.

Copper fill zones are mentioned in the milestone success criteria, but zones don't exist anywhere in the Rust world model (`cypcb-world` has no Zone/CopperFill type). We cannot render what doesn't exist in the data pipeline. This must be either scoped out of S01 or treated as a separate data model addition. Recommendation: defer copper fills to a later slice — S01 should focus on the rendering quality of elements that already have data.

## Recommendation

Rewrite the rendering layer in `renderer.ts` with these additions, in priority order:

1. **Component body outlines** — draw rectangles from `body_width_nm`/`body_height_nm` in silkscreen color (yellow `#C8C800`). Data exists.
2. **Pad pin numbers** — render `pad.number` inside/near each pad, scaled to pad size, with LOD cutoff. Data exists.
3. **Net labels on traces** — render `trace.net_name` at trace midpoint, small font, with LOD cutoff. Data exists.
4. **Refdes upgrade** — font scales with zoom (world-space sizing), not fixed 10px. Data exists.
5. **LOD system** — define 3-4 zoom thresholds that control text density: far (shapes only), medium (refdes + body outlines), close (pad numbers + net labels), very close (all detail). Pure renderer logic.
6. **Pad-to-net lookup** — build `Map<string, string>` from `NetInfo.connections` on snapshot load; key = `"refdes.padnum"`, value = net name. Use for per-pad net highlighting instead of blanket dim.
7. **Drill marks** — crosshair marks on THT pads visible at medium+ zoom. Data exists (`pad.drill_nm`).
8. **Per-layer color refinement** — ensure top copper = red `#C83434`, bottom = blue `#3434C8`, silkscreen = yellow `#C8C800`, through-hole pads = gray `#C8C8C8`, vias = silver `#808080`. Most already correct, just add silkscreen layer drawing.

Extract a `RenderConfig` interface early — this is the contract boundary with S04 (Preferences panel will drive layer colors, font sizes, LOD thresholds).

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Coordinate transforms | `viewport.ts` worldToScreen/screenToWorld | Already correct, battle-tested with Y-flip |
| Pad shape drawing | Existing `drawRoundRect`, `drawOblong` in renderer.ts | All 4 pad shapes (circle, rect, roundrect, oblong) already work |
| Layer color resolution | `layers.ts` getPadColor/getTraceColor | Existing layer mask → color pipeline is correct |
| Net color hashing | `layers.ts` netColor() | Deterministic HSL from net name, with power/ground overrides |
| Geometry helpers | `geometry.ts` pointToSegmentDistance | Used by hit-test, reusable for label placement |
| Theme system | CSS custom properties + getThemeColors() | Light/dark theme plumbing already works |

## Existing Code and Patterns

- `viewer/src/renderer.ts` — 842-line Canvas 2D renderer. Entry point is `render(ctx, state)` called from `main.ts` frame loop. Pattern: each element type has a `draw*` function receiving ctx, viewport, and data. **Reuse**: keep the functional architecture, extend each draw function.
- `viewer/src/types.ts` — BoardSnapshot TypeScript interfaces. `PadInfo` has `number`, `width_nm`, `height_nm`, `shape`, `layer_mask`, `drill_nm`. `TraceInfo` has `net_name`, `width`, `layer`. `ComponentInfo` has `body_width_nm`, `body_height_nm`. **All data for professional rendering exists here.**
- `viewer/src/layers.ts` — Layer colors, visibility, net coloring. `LAYER_COLORS` has `top_silk: '#C8C8C8'` and `bottom_silk: '#808080'` already defined but never used in rendering. **Need to wire up silkscreen drawing.**
- `viewer/src/viewport.ts` — Viewport with scale in px/nm. Key fact: `vp.scale = 0.0001` means 1mm = 100px. LOD thresholds should be defined in terms of `vp.scale`.
- `viewer/src/main.ts` — Orchestrator (1604 lines). Calls `render(ctx, renderState)` at line 924 in `requestAnimationFrame` loop. RenderState built at lines 905-923. **Minimal changes needed here** — just pass new RenderConfig.
- `crates/cypcb-render/src/snapshot.rs` — Rust snapshot types. `PadInfo` has no `net_id` field. `ComponentInfo` has `body_width_nm`/`body_height_nm` (can be zero, fallback computes from pad bbox). **No Rust changes needed for S01.**
- `crates/cypcb-render/src/lib.rs` — `build_snapshot()` constructs `ComponentInfo` with pads from `FootprintLibrary`. Body dimension fallback (pad bbox) exists. **Net info is in `NetInfo.connections` with component+pin mappings.**
- `viewer/src/theme/colors.css` — CSS custom properties for PCB rendering colors (grid, outline, label, background). Light and dark themes defined. **Extend with silkscreen color property if desired.**

## Constraints

- **Canvas 2D only** — per `DECISIONS.md`, 2D rendering uses Canvas API (not WebGL). Acceptable for our component counts.
- **No Rust/WASM changes for S01** — all rendering data exists in the snapshot. Pad-to-net mapping can be built client-side from `NetInfo.connections`.
- **Copper fill zones don't exist in data model** — `cypcb-world` has no Zone/CopperFill type. Cannot render zones. Must be scoped out or deferred.
- **Text rendering is the bottleneck** — Canvas `fillText()` is the slowest part. At 500+ components with pad numbers + net labels, this could be 2000+ `fillText` calls per frame. LOD is essential to manage this.
- **Snapshot doesn't carry silkscreen polylines** — only `body_width_nm`/`body_height_nm` rectangles. Real KiCad silkscreen has curves, text, complex outlines. S01 uses simplified rectangular outlines. Acceptable for beta.
- **Pad-to-net mapping missing from PadInfo** — `PadInfo` has no `net_name`/`net_id`. Must join `NetInfo.connections` → `PadInfo` via `"component.pin"` key on the client side.
- **`body_width_nm`/`body_height_nm` can be zero** — fallback (pad bbox calculation) exists in Rust, but some edge cases may produce 0×0 bodies. Renderer must guard against this.
- **RenderConfig is a boundary contract** — S03 (routing) needs pad highlighting capabilities, S04 (preferences) needs to drive colors/fonts. Design the interface with those consumers in mind.

## Common Pitfalls

- **Fixed-pixel font sizes look wrong at all zoom levels** — Current refdes uses fixed `10px` font. Professional PCB renderers use world-space text sizing (e.g., 0.8mm text that scales with zoom). Set font size as `worldSize * vp.scale` and clamp to readable range (8px–24px).
- **Text rendering performance death spiral** — Drawing 2000+ text labels every frame at 60fps will lag. LOD must cull text below a minimum screen-pixel threshold. Only draw pad numbers when pad width > ~15px on screen. Only draw net labels when trace segment length > ~80px on screen.
- **Canvas font changes are expensive** — Each `ctx.font = '...'` assignment forces font parsing. Batch all text of the same size together. Don't change font per-pad.
- **Pad number text clipping** — Long pad names (e.g., "A1", "VCC") won't fit inside small pads. Use `ctx.measureText()` and fall back to drawing outside the pad if text width > pad width.
- **Body outline rotation** — Component body outline must rotate with the component (`rotation_mdeg`). Current pad rotation logic handles this — reuse the same `ctx.translate`/`ctx.rotate` pattern.
- **Z-ordering of text** — Pad numbers and refdes can overlap with traces and other pads. Render all copper/shapes first, then all text in a separate pass to ensure readability.
- **Net highlighting regression** — Currently all pads dim when net is highlighted (per `DECISIONS.md`). Upgrading to per-pad highlighting is a behavioral change. Test thoroughly.

## Open Risks

- **Canvas text performance at 500+ components** — Need to verify that LOD-culled text rendering stays under 16ms frame budget. Mitigation: prototype with a synthetic 500-component board early, measure before committing to architecture.
- **Snapshot `body_width_nm`/`body_height_nm` accuracy** — These come from footprint bounds, not actual silkscreen geometry. Some footprints may produce visually wrong body outlines (too large, too small). Acceptable for beta but may need footprint-specific silkscreen data later.
- **KiCad visual comparison subjectivity** — "Visually comparable to KiCad" is the success criterion. Without an automated pixel comparison, this is a judgment call. Define specific checkable items: pad numbers visible? net labels visible? layer colors correct? body outlines present? Drill marks visible?
- **RenderConfig interface stability** — S03 and S04 depend on this. If the interface changes significantly during S01, downstream slices need adjustment. Mitigate by designing the interface based on known S03/S04 needs from the boundary map.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| PCB/EDA design | `tscircuit/skill@tscircuit` (157 installs) | available — tscircuit is a different EDA paradigm (React-based); limited relevance for Canvas 2D rendering |
| PCB/EDA design | `l3wi/claude-eda@eda-pcb` (56 installs) | available — may have useful PCB rendering patterns |
| Canvas 2D | `markdown-viewer/skills@canvas` (63 installs) | available — general canvas rendering, not PCB-specific |
| KiCad file format | `o2scale/electronics-agent-kit@kicad-file-format` (26 installs) | available — may help with understanding KiCad visual conventions |

None are directly relevant enough to install. The rendering work is domain-specific to our Canvas 2D PCB renderer — no external skill will meaningfully accelerate it.

## Relevant Requirements

This slice directly supports:
- **UI-09** — Canvas renderer theme syncs with application theme (layer colors + theme colors)

This slice creates the rendering foundation that enables:
- **LIB-08** — User can preview footprints before adding to board (requires professional pad/body rendering)
- **EDIT-10** — Editor and board viewer display side-by-side (viewer quality upgrade)

## Data Flow Analysis

```
.cypcb source → Rust parser → BoardWorld (ECS) → build_snapshot() → BoardSnapshot (JSON/JsValue)
    ↓
TypeScript renderer.ts ← types.ts interfaces
    ↓
Canvas 2D drawing
```

**What exists in snapshot (usable now):**
- Pad shapes with numbers, positions, sizes, drill info, layer masks ✓
- Trace segments with net names, widths, layers ✓
- Via positions with drill/outer diameter, net names ✓
- Component positions, rotation, body dimensions ✓
- Net info with pin connections (for pad-to-net join) ✓
- Ratsnest lines ✓
- DRC violations ✓

**What's missing (cannot render in S01):**
- Copper fill zones (no data model at all)
- Silkscreen polylines (only rectangular body bounds available)
- Solder mask apertures
- Board outline as polygon (only rectangular)

## Sources

- Current renderer analysis: `viewer/src/renderer.ts` (842 lines, functional architecture)
- Snapshot data model: `crates/cypcb-render/src/snapshot.rs` (453 lines)
- DECISIONS.md: "Pad dimming is global when net is highlighted" — to be upgraded with client-side pad-to-net lookup
- DECISIONS.md: "MVP silkscreen uses crosshair markers (+) instead of full text rendering" — original decision for silkscreen, body rectangles are an upgrade from this
- Layer colors defined in `viewer/src/layers.ts` — top_silk and bottom_silk colors exist but aren't rendered
