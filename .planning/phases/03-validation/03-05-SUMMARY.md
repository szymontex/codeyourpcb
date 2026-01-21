---
phase: 03-validation
plan: 05
title: Clearance Checking Rule
subsystem: validation
tags: [rust, drc, spatial-index, clearance]

dependency-graph:
  requires: [03-01, 03-03]
  provides: [ClearanceRule implementation, spatial() method on BoardWorld]
  affects: [03-06, 03-08, 04-export]

tech-stack:
  added: [rstar (already workspace dep)]
  patterns: [two-phase spatial checking, AABB distance calculation]

key-files:
  created:
    - crates/cypcb-drc/src/rules/clearance.rs
  modified:
    - crates/cypcb-drc/Cargo.toml
    - crates/cypcb-world/src/world.rs
    - Cargo.lock

decisions:
  - id: spatial-method-boardworld
    choice: Added spatial() method to BoardWorld returning &SpatialIndex
    rationale: Enables direct spatial index access for DRC rules without going through ECS
  - id: two-phase-checking
    choice: AABB query then exact distance calculation
    rationale: O(log n) candidate selection, then precise distance for violations
  - id: canonical-pair-ordering
    choice: Always order entity pairs (smaller, larger)
    rationale: Prevents duplicate A-B/B-A violation reports
  - id: i128-overflow-prevention
    choice: Use i128 intermediates for distance squared calculation
    rationale: Nanometer values squared can overflow i64

metrics:
  tasks: 2/2
  tests: 12 clearance-specific
  duration: ~3 minutes
  completed: 2026-01-21
---

# Phase 3 Plan 5: Clearance Checking Rule Summary

**One-liner:** ClearanceRule implementation using R*-tree spatial index for O(log n) copper clearance checking with layer filtering and duplicate prevention.

## What Was Built

Implemented the core clearance checking DRC rule (DRC-01 requirement) that detects copper features closer than the minimum clearance specified by design rules.

### ClearanceRule Implementation

Located in `crates/cypcb-drc/src/rules/clearance.rs`:

1. **Two-Phase Spatial Checking Algorithm:**
   - Phase 1: R*-tree query to find candidates within min_clearance radius
   - Phase 2: Exact AABB distance calculation for candidates

2. **Filtering Logic:**
   - Self-exclusion (skip entity == candidate)
   - Layer filtering (skip if layers_overlap returns false)
   - Canonical pair ordering (skip if already checked B-A)

3. **Helper Functions:**
   - `canonical_pair(a, b)` - Orders entity indices for deduplication
   - `aabb_distance(a, b)` - Minimum distance between AABBs
   - `aabb_center(aabb)` - Center point for violation location

### BoardWorld Enhancement

Added `spatial()` method to BoardWorld:
```rust
pub fn spatial(&self) -> &SpatialIndex {
    self.world.resource::<SpatialIndex>()
}
```

This enables DRC rules to access the spatial index directly for iteration and queries.

## Test Coverage

12 unit tests covering:
- No violation when pads are far apart (10mm gap)
- Violation detected when pads too close (0.1mm gap vs 0.15mm rule)
- No violation for overlapping pads on different layers
- No duplicate violations (A-B not reported twice as B-A)
- AABB distance calculation (no overlap, touching, overlapping, diagonal)
- Canonical pair ordering
- AABB center calculation

## Commits

| Hash | Description |
|------|-------------|
| 6c911de | feat(03-05): implement ClearanceRule with spatial index |

## Technical Details

### Algorithm Complexity
- **Time:** O(n log n) for n entities - each entity does O(log n) R*-tree query
- **Space:** O(n) for checked_pairs HashSet in worst case (all pairs violate)

### Distance Calculation

The `aabb_distance` function calculates Euclidean distance between two AABBs:
```rust
// Gap in each dimension (0 if overlapping)
let dx = (a.min.x.max(b.min.x) - a.max.x.min(b.max.x)).max(0);
let dy = (a.min.y.max(b.min.y) - a.max.y.min(b.max.y)).max(0);

// Euclidean distance with i128 to prevent overflow
let dist_sq = (dx as i128) * (dx as i128) + (dy as i128) * (dy as i128);
(dist_sq as f64).sqrt() as i64
```

### Layer Filtering

Uses `SpatialEntry::layers_overlap(mask)` which performs bitwise AND:
```rust
self.layer_mask & mask != 0
```

This correctly handles:
- SMD pads (top only: 0b01)
- SMD pads (bottom only: 0b10)
- Through-hole pads (both: 0b11)

### Dependencies Added

Added `rstar = { workspace = true }` to cypcb-drc Cargo.toml for AABB type access.

## Deviations from Plan

### Rule 3 - Blocking Fix: Added spatial() method

The plan expected a `world.spatial()` method but it didn't exist. Added the method to BoardWorld to enable ClearanceRule to access the spatial index:

- **Found during:** Task 1 implementation
- **Issue:** Plan referenced `world.spatial()` but method didn't exist
- **Fix:** Added `spatial() -> &SpatialIndex` to BoardWorld
- **Files modified:** crates/cypcb-world/src/world.rs
- **Commit:** 6c911de

## Next Phase Readiness

Ready for:
- Plan 03-06 (Drill Size Rule) - Can use similar spatial index patterns
- Plan 03-08 (run_drc Integration) - ClearanceRule now returns real violations

**Blocking issues:** None

**Note:** Same-net exemption is marked TODO. Currently reports violations for same-net items. Future enhancement could add NetId checking.
