---
phase: 03-validation
plan: 01
title: DRC Crate Setup
subsystem: validation
tags: [rust, drc, ecs, traits]

dependency-graph:
  requires: [01-04, 01-05, 02-01]
  provides: [cypcb-drc crate, DrcViolation type, DrcRule trait, DesignRules config]
  affects: [03-02, 03-03, 03-04, 03-05, 03-06]

tech-stack:
  added: [hashbrown 0.15]
  patterns: [trait-based rules, manufacturer presets]

key-files:
  created:
    - crates/cypcb-drc/Cargo.toml
    - crates/cypcb-drc/src/lib.rs
    - crates/cypcb-drc/src/violation.rs
    - crates/cypcb-drc/src/rules/mod.rs
  modified:
    - Cargo.toml

decisions:
  - id: drc-rule-trait
    choice: Object-safe DrcRule trait with name() and check() methods
    rationale: Allows flexible rule composition in Vec<Box<dyn DrcRule>>
  - id: design-rules-structs
    choice: Rust structs for manufacturer presets (not config files)
    rationale: Type-safe, no parsing overhead, simpler
  - id: violation-fields
    choice: DrcViolation with kind, location, entity, other_entity, source_span, message
    rationale: Supports click-to-zoom, error display, and DSL integration

metrics:
  tasks: 3/3
  tests: 17 unit + 7 doc
  duration: ~5 minutes
  completed: 2026-01-21
---

# Phase 3 Plan 1: DRC Crate Setup Summary

**One-liner:** Foundation DRC crate with DrcViolation type, DrcRule trait, and manufacturer preset DesignRules structs.

## What Was Built

Created the `cypcb-drc` crate as the foundation for Design Rule Checking in Phase 3. This establishes:

1. **DrcViolation type** - Captures violation details:
   - `kind`: ViolationKind enum (Clearance, TraceWidth, DrillSize, UnconnectedPin, ViaDrill, AnnularRing)
   - `location`: Point for click-to-zoom in viewer
   - `entity`: Primary ECS entity involved
   - `other_entity`: Secondary entity for clearance violations
   - `source_span`: Optional Span for DSL error display
   - `message`: Human-readable description

2. **Constructor methods** for common violations:
   - `DrcViolation::clearance(entity, other, actual, required, location)`
   - `DrcViolation::drill_size(entity, actual, required, location)`
   - `DrcViolation::unconnected_pin(entity, pin, refdes, location)`

3. **DrcRule trait** - Object-safe interface for rule implementations:
   - `name() -> &'static str` - Rule identifier
   - `check(&self, world, rules) -> Vec<DrcViolation>` - Execute rule

4. **DesignRules struct** - Manufacturer presets:
   - `jlcpcb_2layer()` - 6mil clearance, 0.3mm drill
   - `jlcpcb_4layer()` - 4mil clearance, 0.2mm drill
   - `pcbway_standard()` - 6mil clearance, 0.2mm drill
   - `prototype()` - Relaxed rules for beginners

5. **DrcResult struct** with:
   - `violations: Vec<DrcViolation>`
   - `duration_ms: u64`
   - `passed() -> bool`
   - `violation_count() -> usize`

6. **Placeholder rule structs**:
   - `ClearanceRule`
   - `MinDrillSizeRule`
   - `UnconnectedPinRule`

## Test Coverage

- 17 unit tests covering:
  - DrcResult passed/violated states
  - All DesignRules presets
  - DrcRule trait object safety
  - Rule trait object composition
  - Violation constructor methods
  - ViolationKind display formatting
- 7 passing doc tests for public API examples

## Commits

| Hash | Description |
|------|-------------|
| d9ce257 | feat(03-01): create cypcb-drc crate structure |

## Decisions Made

### DRC Rule as Trait
Chose object-safe trait pattern following 03-RESEARCH.md Pattern 1. Allows rules to be stored in `Vec<Box<dyn DrcRule>>` for flexible composition and parallel execution with rayon in future.

### Rust Structs for Presets
Design rules as typed Rust structs instead of config files (TOML/JSON). Benefits:
- Compile-time type checking
- No parsing overhead
- Simpler code path
- Factory methods for manufacturer presets

### ViolationKind Enum
Created comprehensive enum covering all planned rule types:
- Clearance (Phase 3.3)
- DrillSize (Phase 3.4)
- UnconnectedPin (Phase 3.4)
- TraceWidth, ViaDrill, AnnularRing (future expansion)

## Deviations from Plan

None - plan executed exactly as written.

## Technical Notes

### Dependencies
- Added `hashbrown = "0.15"` for efficient HashSet (will be used in clearance checking to avoid duplicate pair checks)
- Uses `cypcb-parser` for Span type (source location tracking)
- Uses `cypcb-world` for BoardWorld and ECS types

### Feature Flags
- `default = []` - No features by default
- `parallel = ["rayon"]` - Optional parallel rule execution

### API Design
The `run_drc()` function creates all rule checkers and runs them sequentially. With the `parallel` feature, this can be parallelized using rayon.

## Next Phase Readiness

Ready for Phase 3 Plan 2 (Manufacturer Presets DSL) and Phase 3 Plan 3 (Clearance Rule Implementation).

**Blocking issues:** None
**Dependencies resolved:** DrcRule trait, DrcViolation type, DesignRules struct all available for rule implementations.
