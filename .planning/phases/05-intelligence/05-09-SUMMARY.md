---
phase: 05-intelligence
plan: 09
subsystem: workflow
tags: [cli, routing, viewer, freerouting]

dependency-graph:
  requires: ["05-06"]
  provides: ["cli-route-command", "viewer-routing-ui", "routes-loading"]
  affects: ["06-export", "viewer-usage"]

tech-stack:
  added: []
  patterns: ["cli-workflow", "progress-reporting", "file-watching"]

file-tracking:
  key-files:
    created:
      - crates/cypcb-cli/src/commands/route.rs
      - examples/blink.dsn
    modified:
      - crates/cypcb-cli/src/commands/mod.rs
      - crates/cypcb-cli/src/main.rs
      - crates/cypcb-cli/Cargo.toml
      - crates/cypcb-render/src/lib.rs
      - viewer/index.html
      - viewer/src/main.ts
      - viewer/src/wasm.ts

decisions: []

metrics:
  duration: "~15 minutes"
  completed: "2026-01-22"
---

# Phase 05 Plan 09: Autoroute Integration Summary

CLI route command integrating FreeRouting with progress output, viewer Route button with cancel support, and routes file loading.

## What Was Built

### Task 1: CLI Route Command
Created comprehensive `route.rs` command for the CLI:

- **Workflow**: Parse .cypcb -> Build world -> Export DSN -> Run FreeRouting -> Import SES -> Save .routes
- **Progress output**: Shows pass number, routed/unrouted connection counts, elapsed time
- **Dry-run mode**: `--dry-run` exports DSN only for manual FreeRouting usage
- **Timeout control**: `--timeout` flag (default 300 seconds)
- **Max passes**: `--max-passes` to limit routing iterations
- **Clear error messages**: Instructions for installing FreeRouting when JAR not found
- **Environment variable support**: FREEROUTING_JAR for JAR path

Example usage:
```bash
# Full routing workflow
cypcb route design.cypcb --freerouting /path/to/freerouting.jar

# Export DSN for manual routing
cypcb route design.cypcb --dry-run

# With timeout and max passes
cypcb route design.cypcb --timeout 600 --max-passes 10
```

### Task 2: Viewer Routing UI
Added routing integration to the viewer:

- **Route button**: Green button in toolbar to trigger routing
- **Cancel button**: Red cancel button appears during routing
- **Auto-route checkbox**: Enable/disable automatic routing on file save
- **Progress overlay**: Central overlay showing routing progress (pass/routed/unrouted/elapsed)
- **Keyboard shortcut**: Escape key cancels routing
- **Status updates**: Status bar shows routing progress and completion

UI elements added:
- Route button with routing state styling
- Cancel button (hidden until routing starts)
- Auto-route toggle checkbox
- Routing status overlay with spinner animation

Note: Viewer routing is simulated for MVP (real backend integration deferred).

### Task 3: Routes File Loading
Added `load_routes` method to PcbEngine in cypcb-render:

- **Routes file format**: Simple text format with segment/via lines
- **Segment parsing**: `segment net_id layer width_nm x1 y1 x2 y2`
- **Via parsing**: `via net_id x y drill_nm start_layer end_layer`
- **Clear autorouted**: Clears existing autorouted traces before loading new routes
- **Layer parsing**: Supports TopCopper, BottomCopper, Inner(n) formats
- **Error reporting**: Returns parse errors for invalid lines

### Task 4: Workflow Verification
Verified end-to-end dry-run workflow:

- DSN export works correctly for blink.cypcb example
- Board boundary, components, nets exported properly
- Footprint library loads built-in footprints (fixed default() -> new())

## Key Decisions

1. **Routes file format**: Simple text format chosen over JSON for readability and debuggability
2. **Viewer routing simulation**: Real backend communication deferred; UI framework is ready
3. **Dry-run mode**: Allows users to manually run FreeRouting with exported DSN files
4. **Progress format**: "Pass N: X routed, Y unrouted (Z sec)" for clear status

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed FootprintLibrary initialization**
- **Found during:** Task 4 (workflow verification)
- **Issue:** `FootprintLibrary::default()` creates empty library, but built-in footprints needed
- **Fix:** Changed to `FootprintLibrary::new()` which registers builtin SMD/THT/gullwing footprints
- **Files modified:** crates/cypcb-cli/src/commands/route.rs
- **Commit:** 771380b

## Commits

| Hash | Type | Description |
|------|------|-------------|
| fae0213 | feat | add CLI route command for FreeRouting integration |
| 19e5f9f | feat | add routing UI integration in viewer |
| c91e6af | feat | add load_routes method to PcbEngine |
| 771380b | test | verify route workflow with dry-run export |

## Test Results

- CLI unit tests: 3 passed (route command argument parsing)
- CLI integration tests: 9 passed
- cypcb-render tests: 15 passed
- cypcb-router tests: 13 passed (1 ignored - requires FreeRouting)

## Files Summary

| File | Changes |
|------|---------|
| crates/cypcb-cli/src/commands/route.rs | +398 lines - Full routing workflow |
| crates/cypcb-cli/src/commands/mod.rs | +2 lines - Export route command |
| crates/cypcb-cli/src/main.rs | +6 lines - Add route subcommand |
| crates/cypcb-cli/Cargo.toml | +3 lines - Add dependencies |
| crates/cypcb-render/src/lib.rs | +174 lines - Routes loading and layer parsing |
| viewer/index.html | +83 lines - Routing UI styles and elements |
| viewer/src/main.ts | +92 lines - Routing integration |
| viewer/src/wasm.ts | -2 lines - Remove unused field |

## Next Steps

1. Implement real routing backend communication (WebSocket or REST)
2. Add route file watching for automatic reload
3. Handle route invalidation when source changes
4. Add routing statistics panel in viewer
