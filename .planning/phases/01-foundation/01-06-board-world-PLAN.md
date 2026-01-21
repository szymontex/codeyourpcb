---
phase: 01-foundation
plan: 06
type: execute
wave: 3
depends_on: ["01-04"]
files_modified:
  - crates/cypcb-world/src/lib.rs
  - crates/cypcb-world/src/world.rs
  - crates/cypcb-world/src/spatial.rs
  - crates/cypcb-world/src/registry.rs
autonomous: true

must_haves:
  truths:
    - "BoardWorld wraps bevy_ecs::World"
    - "Spatial index enables O(log n) region queries"
    - "Net names are interned to NetId for fast comparison"
  artifacts:
    - path: "crates/cypcb-world/src/world.rs"
      provides: "BoardWorld struct with entity management"
      exports: ["BoardWorld"]
      min_lines: 80
    - path: "crates/cypcb-world/src/spatial.rs"
      provides: "R*-tree spatial index"
      exports: ["SpatialIndex", "SpatialEntry"]
    - path: "crates/cypcb-world/src/registry.rs"
      provides: "NetRegistry for name interning"
      exports: ["NetRegistry"]
  key_links:
    - from: "crates/cypcb-world/src/world.rs"
      to: "bevy_ecs::World"
      via: "struct field"
      pattern: "world: World"
    - from: "crates/cypcb-world/src/spatial.rs"
      to: "rstar::RTree"
      via: "struct field"
      pattern: "RTree<SpatialEntry>"
---

<objective>
Implement the BoardWorld wrapper and spatial indexing infrastructure.

Purpose: Provide a clean API for managing PCB entities with efficient spatial queries needed for DRC and rendering.

Output: BoardWorld with entity spawning, querying, and R*-tree spatial index.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/01-foundation/01-RESEARCH.md

Requirements covered:
- BRD-06: Spatial indexing (R*-tree)
- BRD-03: Net/connection tracking (via NetRegistry)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Implement NetRegistry and SpatialIndex</name>
  <files>
    crates/cypcb-world/src/registry.rs
    crates/cypcb-world/src/spatial.rs
    crates/cypcb-world/Cargo.toml
  </files>
  <action>
Update Cargo.toml to add rstar:
```toml
[dependencies]
rstar = { workspace = true }
```

Create registry.rs with NetRegistry:
```rust
use bevy_ecs::prelude::*;
use std::collections::HashMap;
use crate::components::NetId;

#[derive(Resource, Default)]
pub struct NetRegistry {
    names: Vec<String>,
    lookup: HashMap<String, NetId>,
}

impl NetRegistry {
    pub fn intern(&mut self, name: &str) -> NetId {
        if let Some(&id) = self.lookup.get(name) {
            return id;
        }
        let id = NetId(self.names.len() as u32);
        self.names.push(name.to_string());
        self.lookup.insert(name.to_string(), id);
        id
    }

    pub fn name(&self, id: NetId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (NetId, &str)> {
        self.names.iter().enumerate()
            .map(|(i, s)| (NetId(i as u32), s.as_str()))
    }
}
```

Create spatial.rs with R*-tree integration:
```rust
use bevy_ecs::prelude::*;
use rstar::{RTree, RTreeObject, AABB};
use cypcb_core::{Nm, Point};

#[derive(Debug, Clone)]
pub struct SpatialEntry {
    pub entity: Entity,
    pub envelope: AABB<[i64; 2]>,
    pub layer_mask: u32,
}

impl RTreeObject for SpatialEntry {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

#[derive(Resource, Default)]
pub struct SpatialIndex {
    tree: RTree<SpatialEntry>,
}

impl SpatialIndex {
    pub fn query_region(&self, min: Point, max: Point) -> impl Iterator<Item = Entity> + '_ {
        let envelope = AABB::from_corners([min.x.0, min.y.0], [max.x.0, max.y.0]);
        self.tree.locate_in_envelope_intersecting(&envelope)
            .map(|e| e.entity)
    }

    pub fn query_point(&self, point: Point) -> impl Iterator<Item = Entity> + '_ {
        let p = [point.x.0, point.y.0];
        let envelope = AABB::from_point(p);
        self.tree.locate_in_envelope_intersecting(&envelope)
            .map(|e| e.entity)
    }

    pub fn rebuild(&mut self, entries: Vec<SpatialEntry>) {
        self.tree = RTree::bulk_load(entries);
    }

    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    pub fn len(&self) -> usize {
        self.tree.size()
    }
}
```
  </action>
  <verify>
`cargo build -p cypcb-world` compiles
SpatialIndex can bulk-load entries and query regions
  </verify>
  <done>NetRegistry and SpatialIndex implemented</done>
</task>

<task type="auto">
  <name>Task 2: Implement BoardWorld</name>
  <files>
    crates/cypcb-world/src/world.rs
    crates/cypcb-world/src/lib.rs
  </files>
  <action>
Create world.rs with BoardWorld:
```rust
use bevy_ecs::prelude::*;
use cypcb_core::{Nm, Point, Rect};
use crate::components::*;
use crate::registry::NetRegistry;
use crate::spatial::{SpatialIndex, SpatialEntry};

pub struct BoardWorld {
    world: World,
}

impl BoardWorld {
    pub fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(SpatialIndex::default());
        world.insert_resource(NetRegistry::default());
        Self { world }
    }

    // Board creation
    pub fn set_board(&mut self, name: String, size: (Nm, Nm), layers: u8) -> Entity {
        self.world.spawn((
            Board,
            BoardSize { width: size.0, height: size.1 },
            LayerStack { count: layers },
            crate::components::metadata::Name(name),
        )).id()
    }

    // Component spawning
    pub fn spawn_component(&mut self, /* params */) -> Entity { ... }

    // Query methods
    pub fn components(&self) -> QueryIter<...> { ... }
    pub fn get_component(&self, entity: Entity) -> Option<...> { ... }

    // Net registry access
    pub fn intern_net(&mut self, name: &str) -> NetId {
        self.world.resource_mut::<NetRegistry>().intern(name)
    }

    // Spatial index
    pub fn rebuild_spatial_index(&mut self) { ... }
    pub fn query_region(&self, bounds: Rect) -> Vec<Entity> { ... }

    // ECS access for advanced use
    pub fn ecs(&self) -> &World { &self.world }
    pub fn ecs_mut(&mut self) -> &mut World { &mut self.world }
}
```

Implement all methods with proper error handling.

Update lib.rs:
- Declare modules: components, world, registry, spatial
- Re-export BoardWorld and all components
- Add crate-level documentation
  </action>
  <verify>
`cargo build -p cypcb-world` compiles
`cargo test -p cypcb-world` passes
BoardWorld can spawn and query entities
  </verify>
  <done>BoardWorld provides clean API over ECS</done>
</task>

</tasks>

<verification>
- BoardWorld initializes with empty spatial index and net registry
- Components can be spawned and queried
- Spatial index can be rebuilt and queried by region
- Net names are interned to u32 IDs
- All methods use i64 nanometer coordinates
</verification>

<success_criteria>
1. BoardWorld wraps bevy_ecs::World cleanly
2. SpatialIndex uses rstar R*-tree
3. NetRegistry interns names to NetId(u32)
4. Query methods return correct entities
5. Spatial queries are O(log n)
</success_criteria>

<output>
After completion, create `.planning/phases/01-foundation/01-06-SUMMARY.md`
</output>
