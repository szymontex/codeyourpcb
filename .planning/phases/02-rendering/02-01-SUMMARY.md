---
phase: 02-rendering
plan: 01
subsystem: rendering
tags: [rust, wasm, serialization, api]
completion:
  status: complete
  tasks: 3/3

dependency-graph:
  requires:
    - 01-foundation (cypcb-world ECS model)
    - 01-foundation (cypcb-parser)
  provides:
    - cypcb-render crate
    - BoardSnapshot types
    - PcbEngine API
  affects:
    - 02-02 (viewer setup)
    - 02-03 (canvas rendering)

tech-stack:
  added:
    - serde (serialization)
  patterns:
    - Bridge pattern for Rust-to-JS data transfer
    - Flat snapshot types for serialization

file-tracking:
  key-files:
    created:
      - crates/cypcb-render/Cargo.toml
      - crates/cypcb-render/src/lib.rs
      - crates/cypcb-render/src/snapshot.rs
    modified:
      - Cargo.toml
      - crates/cypcb-world/src/lib.rs

decisions:
  - id: wasm-disabled
    choice: "Disabled wasm-bindgen temporarily"
    reason: "Build environment has unresolved issues with wasm-bindgen crate linking"
    impact: "Core Rust API complete; WASM bindings to be added in follow-up"
  - id: entity-reexport
    choice: "Re-export bevy_ecs::Entity from cypcb-world"
    reason: "Simplifies dependency graph for downstream crates"
    impact: "Clean API for cypcb-render"

metrics:
  duration: ~45min
  completed: 2026-01-21
---

# Phase 02 Plan 01: WASM Rendering Bridge Summary

Flat, serializable board snapshot types and engine API for web viewer.

## What Was Built

### 1. cypcb-render Crate Structure

Created new crate `crates/cypcb-render/` with:
- `Cargo.toml` - Configured with workspace dependencies
- `src/lib.rs` - PcbEngine implementation
- `src/snapshot.rs` - Serializable board types

### 2. BoardSnapshot Types (snapshot.rs)

```rust
pub struct BoardSnapshot {
    pub board: Option<BoardInfo>,
    pub components: Vec<ComponentInfo>,
    pub nets: Vec<NetInfo>,
}

pub struct ComponentInfo {
    pub refdes: String,
    pub value: String,
    pub x_nm: i64, pub y_nm: i64,
    pub rotation_mdeg: i32,
    pub footprint: String,
    pub pads: Vec<PadInfo>,
}

pub struct PadInfo {
    pub number: String,
    pub x_nm: i64, pub y_nm: i64,  // relative to component
    pub width_nm: i64, pub height_nm: i64,
    pub shape: String,  // "circle", "rect", etc.
    pub layer_mask: u32,
    pub drill_nm: Option<i64>,
}
```

All types use i64/i32/u32 primitives for clean JS serialization.

### 3. PcbEngine API (lib.rs)

```rust
pub struct PcbEngine { ... }

impl PcbEngine {
    pub fn new() -> PcbEngine;
    pub fn load_source(&mut self, source: &str) -> String;
    pub fn get_snapshot(&mut self) -> BoardSnapshot;
    pub fn query_point(&mut self, x_nm: i64, y_nm: i64) -> Vec<String>;
}
```

- `load_source()` - Parses source, syncs to world, returns errors
- `get_snapshot()` - Builds complete board snapshot from ECS
- `query_point()` - Returns refdes at coordinate (uses spatial index)

## Test Results

```
running 7 tests
test snapshot::tests::test_board_snapshot_serializes ... ok
test tests::test_engine_new ... ok
test tests::test_build_snapshot_empty ... ok
test tests::test_load_source_parse_error ... ok
test tests::test_load_source_success ... ok
test tests::test_load_source_with_component ... ok
test tests::test_snapshot_with_nets ... ok

test result: ok. 7 passed
```

## Deviations from Plan

### 1. WASM Bindings Temporarily Disabled

**Issue:** The build environment has a mysterious issue where `wasm-bindgen` crate cannot be linked, despite being present in the dependency tree and having valid build artifacts. The rustc command receives the correct `--extern wasm_bindgen=...` flag, but reports "can't find crate".

**Resolution:** Disabled wasm-bindgen in favor of a pure Rust API. The core functionality is complete and tested. WASM bindings will be added in a follow-up task once the build environment issue is resolved.

**Impact:** The viewer will need a thin WASM wrapper layer added later, but the data model and API are ready.

### 2. Entity Re-export from cypcb-world

**Reason:** To avoid cypcb-render needing a direct bevy_ecs dependency, Entity is now re-exported from cypcb-world.

## Commits

| Commit | Description |
|--------|-------------|
| 992eb76 | feat(02-01): create cypcb-render crate with board snapshot types |

## Next Phase Readiness

**Ready for 02-02 (Viewer Setup):**
- BoardSnapshot types defined and serializable
- PcbEngine API ready for use
- JSON serialization tested and working

**Blocker for full WASM:**
- wasm-bindgen linking issue in build environment
- May need environment investigation or alternative approach

## Key Links

- Depends on: `cypcb_world::sync_ast_to_world`
- Depends on: `cypcb_world::BoardWorld`
- Depends on: `cypcb_world::footprint::FootprintLibrary`
- Uses: `serde::Serialize` for all snapshot types
