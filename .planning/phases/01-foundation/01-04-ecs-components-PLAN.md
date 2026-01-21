---
phase: 01-foundation
plan: 04
type: execute
wave: 2
depends_on: ["01-02"]
files_modified:
  - crates/cypcb-world/src/lib.rs
  - crates/cypcb-world/src/components/mod.rs
  - crates/cypcb-world/src/components/position.rs
  - crates/cypcb-world/src/components/electrical.rs
  - crates/cypcb-world/src/components/physical.rs
  - crates/cypcb-world/src/components/metadata.rs
  - crates/cypcb-world/src/components/board.rs
  - crates/cypcb-world/Cargo.toml
autonomous: true

must_haves:
  truths:
    - "All board elements are represented as ECS components"
    - "Position uses cypcb-core Nm coordinates"
    - "Components can be composed flexibly"
  artifacts:
    - path: "crates/cypcb-world/src/components/position.rs"
      provides: "Position, Rotation components"
      exports: ["Position", "Rotation"]
    - path: "crates/cypcb-world/src/components/electrical.rs"
      provides: "NetId, RefDes, Value components"
      exports: ["NetId", "RefDes", "Value"]
    - path: "crates/cypcb-world/src/components/physical.rs"
      provides: "Layer, FootprintRef, Pad components"
      exports: ["Layer", "FootprintRef", "Pad", "PadShape"]
    - path: "crates/cypcb-world/src/components/board.rs"
      provides: "Board marker and properties"
      exports: ["Board", "BoardSize", "LayerStack"]
  key_links:
    - from: "crates/cypcb-world/src/components/position.rs"
      to: "cypcb-core"
      via: "use cypcb_core::{Nm, Point}"
      pattern: "use cypcb_core"
---

<objective>
Define all ECS components for the board model using bevy_ecs.

Purpose: Create composable, cache-friendly data structures that represent all PCB elements (board, components, pads, nets).

Output: Complete component definitions ready for entity spawning.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/01-foundation/01-RESEARCH.md

Requirements covered:
- BRD-01: Component placement -> Position, Rotation
- BRD-02: Multi-layer support -> Layer, LayerStack
- BRD-03: Net tracking -> NetId, NetConnections
- BRD-04: Board outline -> Board, BoardSize
</context>

<tasks>

<task type="auto">
  <name>Task 1: Create position and electrical components</name>
  <files>
    crates/cypcb-world/src/components/mod.rs
    crates/cypcb-world/src/components/position.rs
    crates/cypcb-world/src/components/electrical.rs
  </files>
  <action>
Create components/mod.rs declaring submodules and re-exporting all components.

Create position.rs with:
```rust
use bevy_ecs::prelude::*;
use cypcb_core::{Nm, Point};

/// Position in nanometers from board origin (bottom-left)
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position(pub Point);

/// Rotation in millidegrees (0-359999)
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rotation(pub i32);

impl Rotation {
    pub fn from_degrees(deg: f64) -> Self {
        Self(((deg * 1000.0) as i32).rem_euclid(360_000))
    }
    pub fn to_degrees(&self) -> f64 {
        self.0 as f64 / 1000.0
    }
}
```

Create electrical.rs with:
- NetId(u32) - interned net identifier
- RefDes(String) - reference designator (R1, C1, U1)
- Value(String) - component value
- NetConnections - list of (pin_number, net_id) for a component

All components derive: Component, Debug, Clone, Serialize (where appropriate).
  </action>
  <verify>
`cargo build -p cypcb-world` compiles
Components can be used with bevy_ecs::World
  </verify>
  <done>Position and electrical components defined</done>
</task>

<task type="auto">
  <name>Task 2: Create physical and board components</name>
  <files>
    crates/cypcb-world/src/components/physical.rs
    crates/cypcb-world/src/components/metadata.rs
    crates/cypcb-world/src/components/board.rs
    crates/cypcb-world/src/lib.rs
    crates/cypcb-world/Cargo.toml
  </files>
  <action>
Update Cargo.toml to depend on cypcb-core:
```toml
[dependencies]
cypcb-core = { path = "../cypcb-core" }
bevy_ecs = { workspace = true }
serde = { workspace = true }
```

Create physical.rs with:
- Layer enum: TopCopper, BottomCopper, Inner(u8), TopSilk, BottomSilk, TopMask, BottomMask, TopPaste, BottomPaste, Outline
- FootprintRef(String) - reference to footprint library entry
- PadShape enum: Circle, Rect, RoundRect, Oblong
- Pad struct: number, shape, size (Nm x Nm), drill (Option<Nm>), layer_mask (u32)

Create metadata.rs with:
- SourceSpan { start_byte, end_byte, start_line, start_column } for error reporting
- ComponentKind enum matching grammar: Resistor, Capacitor, Inductor, IC, LED, Connector, Generic

Create board.rs with:
- Board marker component (unit struct)
- BoardSize { width: Nm, height: Nm }
- LayerStack { count: u8 } (2-32 layers per BRD-02)

Update lib.rs:
- Declare components module
- Re-export all component types
- Add crate documentation
  </action>
  <verify>
`cargo build -p cypcb-world` compiles
`cargo test -p cypcb-world` passes (can add basic tests for Layer iteration)
  </verify>
  <done>All ECS components defined and exported</done>
</task>

</tasks>

<verification>
- All components derive bevy_ecs::Component
- Position uses cypcb_core::Point (i64 nanometers)
- Layer enum covers 2-32 layer boards
- Components are composable (no god-struct)
</verification>

<success_criteria>
1. All board elements have corresponding ECS components
2. Position uses integer nanometers from cypcb-core
3. Layer system supports 2-32 layers
4. Components are minimal and composable
5. No floating-point in position storage
</success_criteria>

<output>
After completion, create `.planning/phases/01-foundation/01-04-SUMMARY.md`
</output>
