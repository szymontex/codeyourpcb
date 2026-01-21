---
phase: 01-foundation
plan: 06
subsystem: world
tags: [ecs, spatial-index, bevy, rstar]
dependency-graph:
  requires: ["01-04"]
  provides: ["BoardWorld", "NetRegistry", "SpatialIndex"]
  affects: ["01-08", "02-01"]
tech-stack:
  added: []
  patterns: ["ECS wrapper", "String interning", "Spatial indexing"]
key-files:
  created:
    - crates/cypcb-world/src/world.rs
    - crates/cypcb-world/src/registry.rs
    - crates/cypcb-world/src/spatial.rs
  modified:
    - crates/cypcb-world/src/lib.rs
    - crates/cypcb-world/src/components/mod.rs
    - crates/cypcb-world/src/components/metadata.rs
decisions:
  - id: DEC-0601
    choice: "BoardWorld wraps bevy_ecs::World directly"
    rationale: "Clean API over raw ECS with PCB-specific methods"
  - id: DEC-0602
    choice: "String interning for net names"
    rationale: "O(1) net comparison via u32 IDs instead of string matching"
  - id: DEC-0603
    choice: "R*-tree via rstar for spatial index"
    rationale: "O(log n) region queries for DRC and rendering"
metrics:
  duration: 6m36s
  completed: 2026-01-21
---

# Phase 01 Plan 06: Board World Summary

**One-liner:** BoardWorld wrapper with R*-tree spatial index and net name interning

## What Was Built

### NetRegistry (registry.rs - 298 lines)
String interning for net names to optimize comparison and storage:
- `intern(name)` - Returns existing or creates new NetId(u32)
- `name(id)` - Reverse lookup from ID to string
- `get(name)` - Lookup without interning
- `iter()` - Enumerate all nets with IDs
- Serializable with `rebuild_lookup()` for deserialization

### SpatialIndex (spatial.rs - 432 lines)
R*-tree wrapper using rstar for O(log n) region queries:
- `SpatialEntry` - Entity + AABB envelope + layer mask
- `query_region(min, max)` - Find entities in rectangle
- `query_point(point)` - Find entities containing point
- `query_region_on_layers(min, max, mask)` - Layer-filtered queries
- `rebuild(entries)` - Bulk load for optimal tree structure

### BoardWorld (world.rs - 736 lines)
High-level API wrapping bevy_ecs::World:

**Board Management:**
- `set_board(name, size, layers)` - Create/update board entity
- `board_info()` - Get size and layer stack
- `board_name()` - Get board identifier

**Component Spawning:**
- `spawn_component()` - Full component with refdes, value, position, etc.
- `spawn_component_with_span()` - Include source location
- `spawn_entity()` - Custom bundles

**Net Registry:**
- `intern_net(name)` - Intern net name to ID
- `net_name(id)` - Get name from ID
- `get_net(name)` - Lookup without interning

**Spatial Queries:**
- `rebuild_spatial_index(footprint_bounds)` - Populate from entities
- `query_region(bounds)` - Find entities in area
- `query_point(point)` - Find entities at point

**Entity Access:**
- `component_count()` - Count entities with RefDes
- `find_by_refdes(name)` - Find by designator
- `components()` - Iterate all components
- `ecs()` / `ecs_mut()` - Direct World access

### Name Component (metadata.rs)
Added `Name(String)` component for named entities (board).

## Implementation Details

### BoardWorld Architecture
```
BoardWorld
  +-- world: bevy_ecs::World
  |     +-- Resource: SpatialIndex
  |     +-- Resource: NetRegistry
  |     +-- Entities (board, components)
  +-- board_entity: Option<Entity>
```

### Spatial Index Integration
The spatial index requires explicit rebuild after entity changes:
```rust
world.rebuild_spatial_index(|footprint_name| {
    // Return Rect bounds for the footprint
    footprint_library.get(footprint_name)
        .map(|fp| fp.bounds)
        .unwrap_or_default()
});
```

This design choice (explicit rebuild vs automatic) optimizes for the batch workflow: parse file -> spawn all entities -> rebuild index once.

## Verification Results

- **69 unit tests passing**
- **46 doc tests passing**
- All success criteria met:
  1. BoardWorld wraps bevy_ecs::World (line 77: `world: World`)
  2. SpatialIndex uses rstar R*-tree (line 139: `tree: RTree<SpatialEntry>`)
  3. NetRegistry interns to NetId(u32) (line 93)
  4. Query methods return correct entities (tested)
  5. Spatial queries are O(log n) (R*-tree property)

## Commits

| Hash | Type | Description |
|------|------|-------------|
| 7bcff23 | feat | Add NetRegistry and SpatialIndex |
| e3c6b4d | feat | Implement BoardWorld wrapper |

## Files Summary

| File | Lines | Purpose |
|------|-------|---------|
| world.rs | 736 | BoardWorld wrapper API |
| spatial.rs | 432 | R*-tree spatial index |
| registry.rs | 298 | Net name interning |
| metadata.rs | +48 | Name component |

## Deviations from Plan

None - plan executed exactly as written.

## Next Phase Readiness

Ready for:
- **01-08 (AST Sync):** BoardWorld provides the target for populating from parsed AST
- **02-01 (Rendering):** Spatial index enables efficient visibility queries

No blockers identified.
