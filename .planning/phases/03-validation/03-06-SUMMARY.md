---
phase: 03-validation
plan: 06
subsystem: drc
tags: [drc, drill, connectivity, trace-width, validation]
completed: 2026-01-21

dependency-graph:
  requires:
    - 03-01 (DRC crate and DrcRule trait)
    - 03-03 (Manufacturer presets with DesignRules)
  provides:
    - MinDrillSizeRule implementation
    - UnconnectedPinRule implementation
    - MinTraceWidthRule placeholder
  affects:
    - Phase 5 (MinTraceWidthRule activation)
    - CLI check command (DRC output)

tech-stack:
  patterns:
    - ECS queries for DRC rule iteration
    - FootprintLibrary lookup for pad/pin data
    - Point arithmetic for pad locations

files:
  created:
    - crates/cypcb-drc/src/rules/drill_size.rs (218 lines)
    - crates/cypcb-drc/src/rules/connectivity.rs (287 lines)
    - crates/cypcb-drc/src/rules/trace_width.rs (109 lines)
  modified:
    - crates/cypcb-drc/src/rules/mod.rs
    - crates/cypcb-drc/src/lib.rs
    - crates/cypcb-drc/src/violation.rs

metrics:
  duration: ~6min
  tests-added: 17
  total-drc-tests: 70
---

# Phase 3 Plan 6: Drill Size, Trace Width, Connectivity Rules Summary

Implemented DRC rules for minimum drill size, unconnected pin detection, and trace width placeholder.

## One-liner

MinDrillSizeRule checks THT pad drills against DesignRules.min_drill_size; UnconnectedPinRule detects pins without net connections via FootprintLibrary+NetConnections; MinTraceWidthRule deferred until Phase 5 adds Trace entities.

## Completed Tasks

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Implement MinDrillSizeRule | 6dacc35 | drill_size.rs, mod.rs, violation.rs |
| 2 | Implement UnconnectedPinRule | a0dbd45 | connectivity.rs, mod.rs |
| 3 | Create MinTraceWidthRule placeholder | 4c78f21 | trace_width.rs, mod.rs |
| 4 | Wire up all rules | 5734605 | lib.rs |

## Implementation Details

### MinDrillSizeRule (DRC-03)

- Queries components with FootprintRef and Position
- Looks up footprint from FootprintLibrary
- For each THT pad (drill.is_some()), checks drill < min_drill_size
- Reports violation with pad location (component position + pad offset)
- SMD pads automatically exempt (no drill)
- Added `with_pad_info()` helper to violation.rs for detailed messages

### UnconnectedPinRule (DRC-04)

- Queries components with NetConnections
- For each pad in footprint, calls `nets.pin_net(&pad.number)`
- Reports unconnected pins as "R1.2" format (refdes.pin)
- Unknown footprints gracefully skipped (caught by sync earlier)

### MinTraceWidthRule (DRC-02)

- Created as documented placeholder
- Traces do not exist in board model (Phase 1-3)
- Returns empty Vec until Phase 5 adds Trace component
- Contains pseudocode for future implementation

### run_drc() Updates

- Now includes all 5 rules: Clearance, MinDrillSize, MinTraceWidth, UnconnectedPin, Keepout
- MinTraceWidthRule returns empty (no traces yet)

## Test Coverage

New tests added (17 total):
- drill_size.rs: 6 tests (violations, no violations, SMD exempt, unknown footprint)
- connectivity.rs: 8 tests (detected, fully connected, all unconnected, IC footprints)
- trace_width.rs: 3 tests (placeholder behavior, all presets)

All 70 cypcb-drc tests passing (including 17 doc tests).

## Deviations from Plan

None - plan executed exactly as written.

## Success Criteria Verification

1. MinDrillSizeRule checks all through-hole pads against min_drill_size - PASS
2. UnconnectedPinRule checks all component pins have net assignments - PASS
3. MinTraceWidthRule exists as documented placeholder (DRC-02 coverage) - PASS
4. All rules use efficient ECS queries - PASS (world.ecs_mut() + query)
5. All rules produce violations with correct location info - PASS
6. run_drc() orchestrates all rules including placeholder - PASS

## Next Phase Readiness

Phase 3 continues with plans 03-07 through 03-09:
- Clearance checking is now complete (03-05)
- Drill size and connectivity rules active
- Trace width rule ready to activate when Phase 5 adds Trace entities
