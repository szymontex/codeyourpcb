# Trace Routing System — Architecture & Behavior

## Overview

Interactive PCB trace routing system inspired by KiCad's PNS (Push-and-Shove) router.
Implemented in TypeScript (viewer) with WASM backend for trace storage.

## Key Files

| File | Purpose |
|------|---------|
| `viewer/src/routing.ts` | Routing state machine, preview, obstacle detection, dodge |
| `viewer/src/direction45.ts` | DIRECTION_45 port: 8 directions, BuildInitialTrace, MouseTrailTracer |
| `viewer/src/dodge.ts` | Obstacle avoidance: reroute around foreign pads |
| `viewer/src/trace-edit.ts` | Segment/corner drag, hit-testing, rectangle selection |
| `viewer/src/trace-optimize.ts` | Trace simplification (Ctrl+L) |
| `viewer/src/interaction.ts` | Mouse/keyboard event wiring, T-junction logic |
| `viewer/src/walkaround.ts` | Hull-based walkaround (legacy, not currently used — dodge.ts replaced it) |
| `viewer/src/renderer.ts` | Preview rendering, obstacle markers, solder mask |

## Routing Behavior (KiCad-compatible)

### Creating Traces
1. **Click on pad** → starts routing from that pad, net inherited
2. **Double-click on existing trace** → starts routing from nearest point on trace
3. **Mouse move** → preview path built with `BuildInitialTrace` (45° constraint)
4. **Click on same-net pad** → completes route
5. **Click on same-net trace** → completes route + splits existing trace (T-junction)
6. **Click on empty space** → adds waypoint
7. **Escape** → cancels route

### 45° Constraint System (`direction45.ts`)

Port of KiCad's `DIRECTION_45` class.

**BuildInitialTrace(p0, p1, dir, startDiagonal, cornerMode):**
- Builds 2-segment path from p0 to p1
- Segment 1: horizontal/vertical
- Segment 2: 45° diagonal (or vice versa if `startDiagonal`)
- `MITERED_45` (default): H/V + 45° with sharp corners
- `MITERED_90`: H/V only (90° corners)

**Mouse Trail Tracer:**
- Tracks mouse movement history
- Compares enclosed area of straight-first vs diagonal-first paths against trail
- Automatically picks best posture
- Locks posture after threshold distance

**Keybindings during routing:**
- `/` — flip posture (straight↔diagonal)
- `Q` — toggle corner mode (45°↔90°)
- `A` — toggle 45° constraint on/off (free angle)
- `F` — flip layer (Top↔Bottom)

### Obstacle Detection & Avoidance

**Detection (`checkRouteObstacles` in routing.ts):**
- Checks preview path against all pads of other nets
- Uses Liang-Barsky segment-rect intersection for rectangular pads
- Uses segment-circle distance for round pads
- Checks trace-to-trace crossing with `segSegDist` (finite segment distance)
- Clearance: 150_000nm (0.15mm) default

**Dodge (`dodge.ts`):**
- When path crosses a foreign pad, tries 4 corner waypoints around it
- Each sub-path built with `BuildInitialTrace` (preserves 45°)
- Picks shortest collision-free candidate
- Falls back to 2-corner routes if 1-corner fails
- When cursor is ON a foreign pad, endpoint snaps to nearest edge

**Behavior:**
- Preview turns RED when colliding → placement blocked
- Red X markers at collision points
- Cannot place waypoint or complete route while colliding

### Magnetic Snap

**Target pads:** Pre-computed at route start for the current net. Dual threshold: world radius (1mm) OR 15px screen, whichever larger.

**Trace snap:** When near an existing trace of the same net, snaps to nearest point on segment. Shows pulsing indicator. Click auto-completes (T-junction).

### T-Junction / Trace Splitting

When completing a route on an existing trace (`splitTraceAtPoint`):
1. Removes original trace
2. Adds trace from original start → junction point
3. Adds trace from junction point → original end
4. New route is added connecting to junction

### Net Validation
- Cannot connect pads of different nets
- Cannot complete route on wrong-net pad
- Clicking wrong-net pad during routing is silently rejected

## Trace Editing

### Segment Drag (`dragSegment` in trace-edit.ts)

- **First/last segment** (touching pad): converted to `dragCorner` on the junction point
- **Middle segment**: line through mouse position in drag direction, intersected with adjacent segment lines → junction points slide

### Corner Drag (`dragCorner` / `dragCornerInternal`)

- Walks backward through segments to find splice point
- Rebuilds path from splice point to new position via `BuildInitialTrace`
- Prefers posture matching existing segment direction

### Collision Check During Edit
- After computing new segments, checks against obstacles
- If collision detected, preview is nulled and commit blocked

### UX
- **Single click** on trace → select
- **Double click** on trace → start routing from that point
- **Drag segment** → slide (middle) or adjust junction (endpoint)
- **Drag corner** → reroute via BuildInitialTrace
- **Delete** → remove selected trace
- **Ctrl+L** → simplify (merge colinear segments only, safe)
- **Ctrl+Z/Y** → undo/redo (all operations via command stack)

## Trace Data Model

```typescript
interface TraceSegmentInfo {
  start_x: number; start_y: number;  // nm
  end_x: number;   end_y: number;    // nm
}

interface TraceInfo {
  id: number;
  segments: TraceSegmentInfo[];
  width: number;      // nm
  layer: string;      // 'Top' | 'Bottom'
  net_name: string;
  locked: boolean;
}
```

All coordinates in nanometers (nm). 1mm = 1_000_000nm.

Traces stored in WASM engine via `add_trace(net, layer, width, flat_segments)`.
`flat_segments` is `[x1,y1,x2,y2, x3,y3,x4,y4, ...]` (pairs of segment endpoints).

## Rendering

### PCB View (`renderer.ts`)
- Solder mask overlay (green, semi-transparent)
- FR4 substrate fill (brown)
- Gold/brass pad color (HASL finish)
- Dark drill holes with plating ring
- Grid as dots (not lines)
- KiCad-inspired color palette

### Trace Rendering
- `lineCap: 'round'` for capsule shape (matches KiCad `DrawSegment`)
- Selection glow, net highlight glow
- Net names inside traces (KiCad style, strokeText)
- Locked indicator (dashed overlay)

### Routing Preview
- Dashed polyline for preview path
- Small circles at bend points (H/V to 45° transition)
- Pulsing circle + crosshair at magnetic snap target
- Red color + X markers when colliding

## BigInt Handling

WASM engine returns `i64` fields as `BigInt` in some JS runtimes.
All snapshot values are sanitized via `JSON.stringify` replacer:
- `deepSanitize()` in routing.ts (on every `updatePreview` call)
- `deepBigIntToNumber()` in wasm.ts (on every `get_snapshot` call)

## Known Limitations

1. **No push-and-shove** — traces don't push existing traces aside (KiCad's main feature)
2. **Dodge only works for pads** — trace-to-trace dodge not implemented (detection works, avoidance doesn't reroute around traces)
3. **dragSegment for endpoint segments** falls back to dragCorner (not perfect KiCad behavior)
4. **No arc segments** — only straight line segments (KiCad supports arcs)
5. **Single-layer routing** — no via insertion during routing yet
6. **optimizeTrace disabled** — was too aggressive, only simplifyTrace (colinear merge) is active

## KiCad Source Reference

Key files in `/workspace/competitors/kicad/`:
- `pcbnew/router/pns_line_placer.cpp` — interactive routing
- `pcbnew/router/pns_line.cpp` — dragSegment45, dragCorner45, Walkaround
- `pcbnew/router/pns_dragger.cpp` — drag state machine
- `pcbnew/router/pns_node.cpp` — collision world
- `pcbnew/router/pns_walkaround.cpp` — obstacle walkaround
- `pcbnew/router/pns_shove.cpp` — push-and-shove
- `pcbnew/router/pns_optimizer.cpp` — trace optimization
- `libs/kimath/src/geometry/direction_45.cpp` — BuildInitialTrace
- `pcbnew/pcb_painter.cpp` — PCB rendering
