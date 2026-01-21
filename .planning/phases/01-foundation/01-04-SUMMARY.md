---
phase: 01-foundation
plan: 04
subsystem: board-model
tags: ["ecs", "bevy", "components", "rust"]
dependency-graph:
  requires: ["01-02"]
  provides: ["ecs-components"]
  affects: ["01-06", "01-08"]
tech-stack:
  added: []
  patterns: ["ECS composition", "marker components", "newtype components"]
key-files:
  created:
    - crates/cypcb-world/src/components/mod.rs
    - crates/cypcb-world/src/components/position.rs
    - crates/cypcb-world/src/components/electrical.rs
    - crates/cypcb-world/src/components/physical.rs
    - crates/cypcb-world/src/components/metadata.rs
    - crates/cypcb-world/src/components/board.rs
  modified:
    - crates/cypcb-world/Cargo.toml
    - crates/cypcb-world/src/lib.rs
decisions:
  - id: ECS-COMP-01
    title: Millidegrees for rotation
    choice: i32 millidegrees (0-359999)
    rationale: Deterministic integer comparison, 0.001 degree precision sufficient for PCB
  - id: ECS-COMP-02
    title: Layer mask as u32
    choice: Bit mask for copper layer membership
    rationale: Fast bitwise operations for layer queries, supports up to 32 layers
  - id: ECS-COMP-03
    title: PinConnection as separate struct
    choice: Vec<PinConnection> in NetConnections
    rationale: Supports mixed pin identifiers (numbers and names like "anode", "cathode")
metrics:
  duration: 5m
  completed: 2026-01-21
---

# Phase 01 Plan 04: ECS Components Summary

ECS component definitions for board model using bevy_ecs with integer nanometer coordinates.

## What Was Built

### Position Components (`position.rs`)

- **Position**: Wrapper around `cypcb_core::Point` for entity placement
- **Rotation**: Integer millidegrees (0-359999) with auto-normalization

Key design: Using `Point` from cypcb-core ensures all position data uses i64 nanometers.

### Electrical Components (`electrical.rs`)

- **NetId**: Interned net identifier (u32)
- **RefDes**: Reference designator with prefix/number parsing (R1 -> "R", 1)
- **Value**: Component value string (10k, 100nF, ATmega328P)
- **NetConnections**: Collection of pin-to-net mappings via `PinConnection`

Key design: Net names are interned to u32 for O(1) comparison and minimal memory.

### Physical Components (`physical.rs`)

- **Layer**: Enum covering TopCopper, BottomCopper, Inner(0-29), silkscreen, mask, paste, outline
- **FootprintRef**: Reference to footprint library entry
- **PadShape**: Circle, Rect, RoundRect, Oblong
- **Pad**: Complete pad definition with shape, size, drill, layer_mask

Key design: Layer mask uses bits 0-31 for copper layers, enabling fast bitwise layer queries.

### Board Components (`board.rs`)

- **Board**: Marker component for the board entity
- **BoardSize**: Width and height in nanometers
- **LayerStack**: Layer count (2-32) per BRD-02 requirement

Key design: LayerStack validates 2-32 range at construction time.

### Metadata Components (`metadata.rs`)

- **SourceSpan**: Byte offsets and line/column for error reporting
- **ComponentKind**: Enum matching DSL grammar (Resistor, Capacitor, IC, etc.)

Key design: SourceSpan converts to `miette::SourceSpan` for rich error display.

## Test Coverage

- 28 unit tests covering all component operations
- 20 doc tests verifying examples compile and work
- Tests for edge cases: rotation normalization, layer masks, pad types

## Requirements Addressed

| Requirement | How Addressed |
|-------------|---------------|
| BRD-01: Component placement | Position, Rotation components |
| BRD-02: Multi-layer support | Layer enum + LayerStack (2-32 layers) |
| BRD-03: Net tracking | NetId, NetConnections components |
| BRD-04: Board outline | Board marker, BoardSize |

## Key Links

- `Position` uses `cypcb_core::Point` (i64 nanometers)
- `BoardSize` uses `cypcb_core::Nm` for dimensions
- `Pad` uses `cypcb_core::Nm` for sizes
- `SourceSpan` implements `From<SourceSpan> for miette::SourceSpan`

## Commits

| Hash | Message |
|------|---------|
| 5a4e228 | feat(01-04): add position and electrical ECS components |
| be9b478 | feat(01-04): add physical, board, and metadata ECS components |

## Deviations from Plan

None - plan executed exactly as written.

## Next Phase Readiness

Ready for:
- **01-06 Board World**: Components are complete for entity spawning
- **01-08 AST Sync**: SourceSpan and ComponentKind ready for parser integration

No blockers identified.
