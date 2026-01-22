---
phase: 05-intelligence
plan: 08
subsystem: viewer
tags: ["rendering", "traces", "ratsnest", "canvas"]
dependency-graph:
  requires: ["05-01", "05-06"]
  provides: ["trace-rendering", "via-rendering", "ratsnest-display"]
  affects: ["visual-verification"]
tech-stack:
  added: []
  patterns: ["layer-ordered-rendering", "star-topology-ratsnest"]
key-files:
  created: []
  modified:
    - crates/cypcb-render/src/snapshot.rs
    - crates/cypcb-render/src/lib.rs
    - viewer/src/types.ts
    - viewer/src/renderer.ts
    - viewer/src/layers.ts
    - viewer/src/wasm.ts
    - viewer/src/main.ts
    - viewer/index.html
decisions:
  - id: trace-segment-info
    choice: TraceSegmentInfo with start/end as f64
    reason: "JavaScript-friendly representation of trace segments"
  - id: layer-ordered-rendering
    choice: Draw bottom traces, top traces, then vias
    reason: "Proper z-order for PCB visualization"
  - id: star-topology-ratsnest
    choice: First pin to all other pins
    reason: "Simple MVP visualization for unrouted connections"
  - id: ratsnest-color
    choice: Gold/yellow (#FFD700)
    reason: "High visibility against copper colors"
metrics:
  duration: "25 minutes"
  completed: "2026-01-22"
---

# Phase 5 Plan 8: Trace and Ratsnest Rendering Summary

Trace and via rendering added to viewer with ratsnest toggle for unrouted connections

## One-liner

Trace polylines with layer colors, via drill holes, and toggleable yellow ratsnest for unrouted nets

## Completed Tasks

| Task | Name | Commit | Key Changes |
|------|------|--------|-------------|
| 1 | Extend BoardSnapshot with Traces | a11528f | TraceInfo, ViaInfo, RatsnestInfo types in snapshot.rs |
| 2 | Implement Trace and Via Rendering | bbf27c0 | drawTrace, drawVia, drawRatsnest functions in renderer.ts |
| 3 | Add Ratsnest Layer Toggle | 754c9c7 | Ratsnest checkbox in toolbar, connected to render state |

## Technical Details

### Rust Changes (cypcb-render)

1. **New Snapshot Types** (snapshot.rs):
   - `TraceSegmentInfo`: start/end coordinates as f64
   - `TraceInfo`: segments, width, layer, net_name, locked
   - `ViaInfo`: position, drill, outer_diameter, net_name
   - `RatsnestInfo`: start/end coordinates, net_name
   - `BoardSnapshot` extended with traces, vias, ratsnest vectors

2. **Collection Methods** (lib.rs):
   - `collect_traces()`: Queries Trace entities, converts to TraceInfo
   - `collect_vias()`: Queries Via entities, converts to ViaInfo
   - `collect_ratsnest()`: Star-topology for nets without traces

### TypeScript Changes (viewer)

1. **Type Definitions** (types.ts):
   - Added TraceSegmentInfo, TraceInfo, ViaInfo, RatsnestInfo interfaces
   - Updated BoardSnapshot to include new fields

2. **Rendering Functions** (renderer.ts):
   - `drawTrace()`: Polyline with layer color, rounded ends, locked indicator
   - `drawVia()`: Filled circle with drill hole cutout
   - `drawRatsnest()`: Thin dashed yellow lines
   - Layer-ordered rendering: bottom -> top -> vias -> ratsnest

3. **Layer Controls** (layers.ts):
   - Added `via` color (#808080 gray)
   - Added `ratsnest` color (#FFD700 gold)
   - Added `getTraceColor()` helper for layer-based coloring

4. **UI Changes** (index.html, main.ts):
   - Added Ratsnest checkbox to toolbar
   - Connected checkbox to showRatsnest render state
   - Added sample traces/ratsnest to MockPcbEngine for testing

## Rendering Order

1. Grid (behind everything)
2. Board outline
3. Bottom copper traces
4. Top copper traces
5. Inner layer traces
6. Component pads
7. Vias (on top of traces)
8. Ratsnest lines (if enabled)
9. DRC violations (top of everything)

## Deviations from Plan

None - plan executed exactly as written.

## Next Phase Readiness

- Traces render with actual width and layer colors
- Vias show copper ring and drill hole
- Ratsnest toggleable via checkbox
- Ready for visual verification checkpoint (05-10)
