---
phase: 05-intelligence
plan: 04
completed: 2026-01-22
duration: 6 minutes
subsystem: routing
tags: [dsn, freerouting, autorouter, export]

depends_on:
  requires: [05-01]
  provides: [dsn-export, routing-types]
  affects: [05-06]

tech_stack:
  added: []
  patterns:
    - "Specctra DSN S-expression format"
    - "nm to mil coordinate conversion"
    - "Locked trace export for FreeRouting"

key_files:
  created:
    - crates/cypcb-router/Cargo.toml
    - crates/cypcb-router/src/lib.rs
    - crates/cypcb-router/src/types.rs
    - crates/cypcb-router/src/dsn.rs
    - crates/cypcb-router/tests/dsn_integration.rs
  modified:
    - Cargo.toml

decisions:
  - id: dsn-mil-resolution
    choice: "mil unit with 10 resolution (0.1 mil)"
    rationale: "Standard FreeRouting resolution, sufficient precision"
  - id: mutable-world-for-export
    choice: "export_dsn takes &mut BoardWorld"
    rationale: "bevy_ecs queries require mutable access for cache initialization"
  - id: locked-trace-fixed-type
    choice: "Export locked traces with (type fix)"
    rationale: "FreeRouting will not modify fixed wires during autorouting"

metrics:
  tests_passing: 29
  lines_of_code: 1511
---

# Phase 5 Plan 4: DSN Export for FreeRouting Summary

Specctra DSN export implemented for FreeRouting autorouter integration via cypcb-router crate.

## One-Liner

Specctra DSN export with full board structure (boundary, layers, components, nets, locked traces) for FreeRouting integration.

## What Was Built

### cypcb-router Crate

New crate providing autorouting integration infrastructure:

**types.rs (334 lines):**
- `RoutingStatus` - Complete/Partial/Failed enum for routing results
- `RouteSegment` - Routed wire segment with net, layer, width, start/end points
- `ViaPlacement` - Via placement with net, position, drill, layers
- `RoutingResult` - Aggregates status, routes, and vias from autorouting

**dsn.rs (712 lines):**
- `DsnExportError` - Error types for export (Io, MissingBoard, EmptyNet)
- `export_dsn()` - Main entry point, writes complete Specctra DSN
- `write_structure()` - Board boundary, layer definitions, design rules
- `write_placement()` - Component positions grouped by footprint
- `write_library()` - Footprint images and padstack definitions
- `write_network()` - Nets with pin connections and net classes
- `write_wiring()` - Locked traces exported as fixed wires
- `nm_to_mil()` - Coordinate conversion (1 mil = 25,400 nm)
- `quote_dsn()` - String quoting with escape handling
- `layer_to_dsn_name()` - Layer enum to DSN layer name (F.Cu, B.Cu, etc.)

### DSN Format Structure

The exported DSN follows Specctra specification:

```
(pcb "board_name"
  (parser
    (string_quote ")
    (space_in_quoted_tokens on)
    (host_cad "CodeYourPCB")
    (host_version "0.1.0")
  )
  (resolution mil 10)
  (unit mil)
  (structure
    (boundary (rect pcb 0 0 width height))
    (layer F.Cu (type signal))
    (layer B.Cu (type signal))
    (rule (width 8) (clearance 6))
  )
  (placement
    (component "footprint"
      (place "refdes" x y side rotation)
    )
  )
  (library
    (image "footprint"
      (pin padstack "pin" x y)
    )
    (padstack name
      (shape ...)
      (attach off)
      (hole round diameter)
    )
  )
  (network
    (net "name" (pins refdes-pin ...))
    (class default ... (rule (width 8) (clearance 6)))
  )
  (wiring
    (wire
      (path layer width x1 y1 x2 y2 ...)
      (net "name")
      (type fix)
    )
  )
)
```

### Integration Tests

Comprehensive test suite in dsn_integration.rs (413 lines):
- Full board export with 3 components and 2 nets
- Board boundary conversion (mm to mil)
- Component placement verification
- Net connection verification
- Padstack definitions
- Layer definitions (F.Cu, B.Cu)
- Design rules export
- Net class grouping
- Locked trace export as fixed wiring
- Rotation export
- Image definitions
- Balanced parentheses validation
- Coordinate conversion accuracy test

## Test Results

```
29 tests passing:
- 16 unit tests (types + dsn)
- 13 integration tests
- 2 doc tests ignored (require file I/O)
```

## Key Decisions

### 1. Mutable BoardWorld for Export

The `export_dsn()` function takes `&mut BoardWorld` instead of `&BoardWorld` because bevy_ecs queries require mutable access for cache initialization. This is a common pattern when iterating ECS entities.

### 2. Mil Resolution

Using mil units with resolution 10 (0.1 mil precision) matches FreeRouting's default and provides sufficient accuracy for PCB routing.

### 3. Locked Trace Export

Only locked traces are exported to the wiring section with `(type fix)`. Unlocked traces are not exported, allowing FreeRouting to route them. This enables incremental routing workflows where critical traces are manually placed first.

### 4. Coordinate Conversion

Conversion from nanometers to mils: `nm / 25,400 = mil`
- 1mm = 1,000,000nm = ~39.37 mil
- 25.4mm = 1000 mil exactly

## Deviations from Plan

None - plan executed exactly as written.

## Next Steps (Plan 05-06)

Plan 05-06 will implement:
- SES (Session) import to read FreeRouting output
- FreeRouting CLI wrapper for subprocess execution
- CLI commands for route/import workflow

## Commits

| Hash | Description |
|------|-------------|
| 8c668cd | feat(05-04): create cypcb-router crate with DSN export |
| 345b6ec | test(05-04): add DSN export integration tests |
