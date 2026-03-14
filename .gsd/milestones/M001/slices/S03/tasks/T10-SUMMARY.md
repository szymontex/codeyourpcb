---
id: T10
parent: S03
milestone: M001
provides: []
requires: []
affects: []
key_files: []
key_decisions: []
patterns_established: []
observability_surfaces: []
drill_down_paths: []
duration: 
verification_result: passed
completed_at: 
blocker_discovered: false
---
# T10: Zones & Keepouts

**# Phase 03 Plan 10: Zones and Keepouts Summary**

## What Happened

# Phase 03 Plan 10: Zones and Keepouts Summary

Rectangular zone regions with keepout and copper pour support, plus KeepoutRule for DRC.

## What Was Built

### DSL Grammar Extensions
Added zone syntax for defining rectangular regions:
```cypcb
keepout antenna_clearance {
    bounds 10mm, 10mm to 20mm, 20mm
    layer top
}

zone gnd_pour {
    bounds 0mm, 0mm to 50mm, 50mm
    layer bottom
    net GND
}
```

### AST Types
- `ZoneKind` enum: `Keepout`, `CopperPour`
- `ZoneDef` struct with bounds, layer, net, name, span
- Extended `Definition` enum with `Zone` variant

### ECS Components
- `Zone` component with `Rect` bounds, `ZoneKind`, layer_mask, optional name
- Methods: `keepout()`, `copper_pour()`, `with_name()`, `contains()`, `on_layer()`, `layers_overlap()`

### DRC Rule
- `KeepoutRule` detects components placed inside keepout zones
- `ViolationKind::KeepoutViolation` for zone violations
- `DrcViolation::keepout()` constructor with zone name in message
- Added to `run_drc()` for automatic checking

## Key Decisions

### DrcRule Trait Mutability
Changed `DrcRule::check(&self, world: &BoardWorld, rules: &DesignRules)` to take `&mut BoardWorld`. This was necessary because bevy_ecs queries need mutable access to World to initialize their internal cache. No actual board data is modified - only the query cache.

### Component Center Point Checking
The keepout rule currently checks if a component's center position is inside a zone. Full footprint bounds checking would require more complex spatial index integration and is deferred to a future enhancement.

## Commits

| Hash | Description |
|------|-------------|
| 483f23e | feat(03-10): add zone/keepout grammar and AST |
| 5206eaa | feat(03-10): add Zone ECS component and AST sync |
| 9130a65 | feat(03-10): implement keepout DRC rule |

## Test Coverage

- **Parser tests**: 4 new tests for keepout zone, copper pour, all layers, anonymous zone
- **Sync tests**: 3 new tests for zone entity creation and layer mask parsing
- **DRC tests**: 5 new tests for violation detection, outside zone, copper pour (ignored), multiple components, unnamed zone

## Files Changed

```
crates/cypcb-parser/grammar/grammar.js   # Zone grammar rules
crates/cypcb-parser/src/ast.rs           # ZoneDef, ZoneKind AST types
crates/cypcb-parser/src/parser.rs        # convert_zone() parser method
crates/cypcb-world/src/components/zone.rs  # NEW: Zone ECS component
crates/cypcb-world/src/components/mod.rs   # Export Zone, ZoneKind
crates/cypcb-world/src/sync.rs             # sync_zone() handler
crates/cypcb-world/src/world.rs            # zones() query method
crates/cypcb-drc/src/violation.rs          # KeepoutViolation, keepout()
crates/cypcb-drc/src/rules/mod.rs          # KeepoutRule implementation
crates/cypcb-drc/src/lib.rs                # run_drc() includes KeepoutRule
```

## Deviations from Plan

### Minor Implementation Differences
1. **Keepout rule in rules/mod.rs**: Plan specified `keepout.rs` but implementation added rule directly to `rules/mod.rs` for simplicity.
2. **Spatial index query**: Plan suggested `query_region_entries()` but implemented with simpler component iteration using `BoardWorld::zones()` and `BoardWorld::components()`.

## Next Phase Readiness

Phase 03 core validation infrastructure is now complete with:
- Design rules and manufacturer presets
- IC footprint library
- Zone/keepout support with DRC checking

Remaining plans in Phase 03 can proceed with the established patterns.
