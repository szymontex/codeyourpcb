---
phase: 05
plan: 01
subsystem: world-model
tags: [ecs, trace, via, grammar, dsl, routing]

dependency-graph:
  requires: []
  provides: [trace-ecs, via-ecs, trace-grammar, net-constraints]
  affects: [05-06-autorouter]

tech-stack:
  added: []
  patterns: [ecs-components, ast-to-ecs-sync]

file-tracking:
  key-files:
    created:
      - crates/cypcb-world/src/components/trace.rs
    modified:
      - crates/cypcb-world/src/components/mod.rs
      - crates/cypcb-world/src/lib.rs
      - crates/cypcb-parser/grammar/grammar.js
      - crates/cypcb-parser/src/ast.rs
      - crates/cypcb-parser/src/parser.rs
      - crates/cypcb-world/src/sync.rs

decisions:
  - id: trace-as-polyline
    choice: "Trace stored as Vec<TraceSegment> polyline"
    rationale: "Efficient representation for Manhattan and diagonal routing"
  - id: locked-trace-flag
    choice: "Boolean locked field on Trace and Via"
    rationale: "Simple flag for autorouter to respect manual traces"
  - id: current-unit-enum
    choice: "CurrentUnit enum with mA/A variants"
    rationale: "Type-safe current values with clear unit conversion"

metrics:
  duration: ~15min
  completed: 2026-01-22
---

# Phase 05 Plan 01: Trace & Via ECS and DSL Extensions Summary

Trace/Via ECS components with polyline paths, plus net constraint grammar (width/clearance/current) and manual trace DSL syntax

## What Was Built

### Task 1: Trace and Via ECS Components

Created `crates/cypcb-world/src/components/trace.rs` with:

- **TraceSegment**: Line segment with start/end points, length/midpoint calculations
- **Trace**: ECS component with segments vec, width, layer, net_id, locked, source
- **Via**: ECS component for drill holes connecting layers with annular ring
- **TraceSource**: Enum distinguishing Manual vs Autorouted traces

Key design decisions:
- Traces stored as polylines (Vec<TraceSegment>) for flexible routing
- i128 used internally for length calculation to prevent overflow
- Via supports blind/buried vias via start_layer/end_layer

### Task 2: Grammar and AST Extensions

Extended Tree-sitter grammar with:

- **Current constraint**: `current 500mA` or `current 2A` on nets
- **Trace definition**: Complete manual trace syntax
  ```cypcb
  trace VCC {
      from R1.1
      to C1.1
      via 5mm, 8mm
      layer Top
      width 0.3mm
      locked
  }
  ```

AST types added:
- `Definition::Trace(TraceDef)` variant
- `TraceDef`: net, from, to, waypoints, layer, width, locked
- `CurrentValue`: value, unit, span with to_milliamps/to_amps conversions
- `CurrentUnit`: Milliamps | Amps enum
- `NetConstraints`: now includes optional current field

### Task 3: Sync Trace to ECS

Extended sync.rs with:

- `sync_trace()`: Converts TraceDef to Trace ECS entity
- `get_pin_position()`: Resolves pin references to positions
- `parse_layer_name()`: Converts "Top"/"Bottom"/"Inner1" to Layer enum

New error types:
- `SyncError::InvalidTracePin`: Unknown component/pin in trace
- `SyncError::MissingNet`: Trace references undefined net
- `SyncError::UnknownLayer`: Invalid layer name

## Commits

| Hash | Description |
|------|-------------|
| 09ccb44 | feat(05-01): add Trace and Via ECS components |
| c505ea3 | feat(05-01): extend grammar with net constraints and manual traces |

Note: Task 3 sync changes were included in a parallel plan's commit (afe3d26).

## Test Results

- 19 trace component tests (TraceSegment, Trace, Via)
- 7 new parser tests (current constraints, trace definitions)
- 5 new sync tests (trace basic, waypoints, locked, errors)

All tests pass:
```
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured
```

## Key Files

| File | Purpose |
|------|---------|
| `crates/cypcb-world/src/components/trace.rs` | Trace, Via, TraceSegment types |
| `crates/cypcb-parser/grammar/grammar.js` | DSL grammar with trace/current rules |
| `crates/cypcb-parser/src/ast.rs` | TraceDef, CurrentValue, CurrentUnit types |
| `crates/cypcb-parser/src/parser.rs` | Parser for traces and current |
| `crates/cypcb-world/src/sync.rs` | sync_trace() and error types |

## Dependencies for Future Plans

- **05-06 (Autorouter)**: Will create Trace entities with TraceSource::Autorouted
- **05-04 (DRC Engine)**: Will use net constraints for clearance checking
- **05-02 (IPC-2221)**: Current values feed into trace width calculator

## Deviations from Plan

None - plan executed exactly as written.

## Next Phase Readiness

Ready for:
- Plan 05-06: Autorouter can now create and store Trace entities
- Plan 05-04: DRC can read net constraints for validation
