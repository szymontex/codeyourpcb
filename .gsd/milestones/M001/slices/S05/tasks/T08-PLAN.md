# T08: Trace & Ratsnest Rendering

**Slice:** S05 — **Milestone:** M001

## Description

Add trace and ratsnest rendering to the viewer so routing results are visible.

Purpose: Users need to see traces after autorouting. Ratsnest shows what still needs routing. Per CONTEXT.md: "Full trace rendering: actual width, copper layer colors, vias visible" and "Ratsnest: toggle option in layer controls".
Output: Traces and vias visible in viewer, ratsnest toggleable

## Must-Haves

- [ ] "Traces render with actual width on copper layers"
- [ ] "Vias render as filled circles with drill"
- [ ] "Ratsnest shows unrouted connections"
- [ ] "Ratsnest can be toggled in layer controls"

## Files

- `crates/cypcb-render/src/snapshot.rs`
- `crates/cypcb-render/src/lib.rs`
- `viewer/src/types.ts`
- `viewer/src/renderer.ts`
- `viewer/src/layers.ts`
