---
phase: 03-validation
plan: 08
completed: 2026-01-21
duration: ~15 minutes

subsystem: viewer
tags: [typescript, canvas, ui, drc-display]

dependency_graph:
  requires:
    - 03-07 (DRC integration provides violations in BoardSnapshot)
  provides:
    - Visual violation markers on canvas
    - Status bar error badge with count
    - Error panel with click-to-zoom
  affects:
    - 03-09 (if visual verification plan exists)

tech_stack:
  added: []
  patterns:
    - VS Code-style status bar error indicator
    - KiCad-style ring markers at violation locations
    - Click-to-zoom navigation pattern

key_files:
  modified:
    - viewer/src/layers.ts (added violation colors)
    - viewer/src/renderer.ts (drawViolation function, RenderState update)
    - viewer/src/main.ts (error badge, error panel, zoomToLocation)
    - viewer/index.html (error-badge, error-panel elements with CSS)
---

# Phase 3 Plan 8: Violation Display Summary

Visual DRC feedback with non-invasive error markers, status bar badge, and click-to-zoom error panel.

## One-liner

KiCad-style red ring markers at violation locations with VS Code-style error badge and panel.

## What Was Built

### Violation Marker Rendering (Task 1)
- Added `violation` and `violation_ring` colors to LAYER_COLORS in layers.ts
- Added `showViolations: boolean` field to RenderState interface
- Implemented `drawViolation()` function rendering:
  - Outer red ring (15px radius, 3px stroke)
  - Inner semi-transparent fill (10px radius, 30% opacity)
- Markers render on top of all other elements

### Status Bar Error Badge (Task 2)
- Added `#error-badge` span with pill-style appearance (red background, white text)
- Badge shows count of violations ("N errors")
- Badge hidden when no violations exist
- `updateErrorBadge()` function manages visibility based on violations array
- Refactored status area to use separate `#status-text` span

### Error Panel with Click-to-Zoom (Task 3)
- Added `#error-panel` overlay (VS Code style):
  - 350px width, max 200px height
  - Header with "DRC Errors" title and close button
  - Scrollable list of error items
- Each error shows `[kind] message` format
- Click error item triggers `zoomToLocation()`:
  - Centers viewport on violation coordinates
  - Zooms to 10mm viewing area around point
- Panel toggles visibility on badge click

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| Fixed 15px marker radius | Screen-space size ensures visibility at any zoom |
| Semi-transparent inner fill | Distinguishes from solid pads, shows underlying geometry |
| Panel below status bar | Non-invasive positioning, doesn't block canvas |
| 5mm zoom margin | Provides context around violation location |

## Commits

| Hash | Message | Files |
|------|---------|-------|
| 62b3eea | feat(03-08): add violation marker rendering | layers.ts, renderer.ts, main.ts |
| 9ea1025 | feat(03-08): add status bar error badge | index.html, main.ts |
| 50968ba | feat(03-08): add error panel with click-to-zoom | index.html, main.ts |

## Deviations from Plan

None - plan executed exactly as written.

## Verification Checklist

- [x] `npm run build` succeeds
- [x] Violation markers render as red rings at violation coordinates
- [x] Status bar badge shows error count when violations exist
- [x] Badge hidden when no violations
- [x] Click badge toggles error panel visibility
- [x] Error panel lists violations with kind and message
- [x] Click error item zooms viewport to violation location
- [x] Non-invasive UI (doesn't block board interaction)

## Next Phase Readiness

Plan 03-08 complete. Phase 3 has 9 of 10 plans done. Remaining:
- 03-09: TBD (if exists)

All DRC display functionality integrated with viewer.
