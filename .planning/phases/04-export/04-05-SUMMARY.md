---
phase: 04-export
plan: 05
completed: 2026-01-28
duration: 539s
subsystem: manufacturing
tags: [bom, cpl, pick-and-place, assembly, csv, jlcpcb]
requires: [04-01-coordinate-conversion]
provides:
  - BOM CSV/JSON export
  - Pick-and-place CSV export
  - Component grouping logic
  - JLCPCB format compatibility
affects: [04-08-zip-packaging, 04-09-cli-integration]
tech-stack:
  added: []
  patterns: [natural-sorting, builder-pattern]
key-files:
  created:
    - crates/cypcb-export/src/bom/mod.rs
    - crates/cypcb-export/src/bom/csv.rs
    - crates/cypcb-export/src/bom/json.rs
    - crates/cypcb-export/src/cpl/mod.rs
    - crates/cypcb-export/src/cpl/csv.rs
  modified:
    - crates/cypcb-export/Cargo.toml
    - crates/cypcb-export/src/lib.rs
    - crates/cypcb-export/src/excellon/writer.rs
decisions:
  - choice: "Comma-separated designators in BOM CSV"
    rationale: "JLCPCB expects single row per component group with comma-separated refs, reduces BOM size"
  - choice: "Natural sorting for designators"
    rationale: "R1, R2, R10 order is more intuitive than lexical R1, R10, R2"
  - choice: "Coordinates with 'mm' suffix in CPL"
    rationale: "JLCPCB CPL format requires explicit unit suffix (e.g., '50.800mm')"
  - choice: "Rotation in integer degrees"
    rationale: "Pick-and-place machines typically use whole degrees, simplifies CSV format"
  - choice: "Empty CSV when no components"
    rationale: "csv crate doesn't write headers without data rows, consistent behavior"
  - choice: "CplConfig for machine variations"
    rationale: "Different machines use different rotation conventions and Y-axis directions"
---

# Phase 4 Plan 5: BOM and Pick-and-Place Export Summary

**One-liner:** BOM and CPL export in JLCPCB-compatible CSV/JSON formats with component grouping and natural sorting

## What Was Built

### Bill of Materials (BOM) Export

**Module:** `crates/cypcb-export/src/bom/`

**Core Functionality:**
- Component grouping by (value, footprint) tuple
- Natural sorting of designators (handles R1, R2, R10 correctly)
- CSV export in JLCPCB format with comma-separated designators
- JSON export with metadata (board name, export date, counts)
- Designator consolidation: `["R1", "R2", "R3"]` → `"R1,R2,R3"`

**CSV Format:**
```csv
Designator,Footprint,Quantity,Comment
R1,R2,R3,0402,3,10k
C1,0402,1,100nF
```

**JSON Format:**
```json
{
  "metadata": {
    "board_name": "MyBoard",
    "export_date": "2026-01-28T14:54:39Z",
    "unique_components": 2,
    "total_components": 4
  },
  "components": [...]
}
```

**Implementation Details:**
- `group_components()` consolidates identical components
- `natural_sort_key()` extracts prefix and number for sorting
- Empty BOM produces empty CSV (csv crate behavior)
- Added `csv` and `serde_json` dependencies

**Test Coverage:** 18 tests
- Component grouping (empty, single, multiple, identical)
- Natural sorting (R1, R2, R10 order)
- CSV format validation
- JSON structure validation
- Metadata generation

### Pick-and-Place (CPL) Export

**Module:** `crates/cypcb-export/src/cpl/`

**Core Functionality:**
- Component position export in millimeters (3 decimal places)
- Rotation converted from millidegrees to degrees
- Layer detection from first pad in footprint
- Natural sorting by designator
- Configuration support for machine variations

**CSV Format:**
```csv
Designator,Mid X,Mid Y,Layer,Rotation
U1,50.800mm,30.480mm,Top,90
R1,12.345mm,67.890mm,Top,0
```

**CplConfig Options:**
- `rotation_offset`: Added to all rotations (for machine conventions)
- `flip_y`: Y-axis flip for machines using Y-down coordinates

**Implementation Details:**
- Component center position (not corner)
- Coordinate conversion: `nm → mm` (divide by 1,000,000)
- Rotation conversion: `millidegrees → degrees` (divide by 1,000)
- Layer detection: checks first pad for TopCopper/BottomCopper
- Sorted output for deterministic file generation

**Test Coverage:** 12 tests
- Empty board handling
- Coordinate conversion accuracy
- Rotation angle conversion
- Rotation offset configuration
- Natural sorting
- CSV header validation

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed excellon test compilation**
- **Found during:** Task 1 testing
- **Issue:** excellon tests missing `drill_type_filter` parameter after 04-04 signature change
- **Fix:** Added `None` parameter to `export_excellon()` calls in tests
- **Files modified:** `crates/cypcb-export/src/excellon/writer.rs`
- **Commit:** Part of task commits

**2. [Rule 1 - Bug] Fixed BOM designator borrow issue**
- **Found during:** Task 1 testing
- **Issue:** `designators` moved before calculating `quantity`
- **Fix:** Calculate `quantity` before moving `designators`
- **Files modified:** `crates/cypcb-export/src/bom/mod.rs`

**3. [Rule 1 - Bug] Fixed CPL temporary borrow issue**
- **Found during:** Task 2 testing
- **Issue:** `CplConfig::default()` temporary dropped while borrowed
- **Fix:** Bind default config to variable with appropriate lifetime
- **Files modified:** `crates/cypcb-export/src/cpl/csv.rs`

## Key Architectural Decisions

### Component Grouping Strategy

**Decision:** Group by (value, footprint) tuple, collect designators
**Alternatives considered:**
- Group by value only (would mix different footprints)
- Group by footprint only (would mix different values)
**Trade-offs:** Chosen approach matches standard BOM practice

### CSV Format Choice

**Decision:** JLCPCB format with comma-separated designators
**Rationale:**
- Industry standard format for Chinese manufacturers
- Reduces BOM line count (1 row vs N rows for N identical components)
- Easy to import into spreadsheets
- Compatible with most assembly services

### Natural Sorting

**Decision:** Parse designator into (prefix, number) for sorting
**Implementation:** `natural_sort_key()` extracts letters and digits separately
**Benefit:** R1, R2, R10 sorts correctly (not R1, R10, R2)

### CPL Configuration

**Decision:** Provide `CplConfig` struct with optional parameters
**Rationale:**
- Different pick-and-place machines use different conventions
- Rotation offset common (0° vs 90° reference)
- Y-axis direction varies (Y-up vs Y-down)
- Builder pattern for ergonomics

### Error Handling

**Decision:** Return `Box<dyn std::error::Error>` for export functions
**Rationale:**
- Allows multiple error types (csv, io, utf8, footprint lookup)
- Simplifies error propagation with `?` operator
- Sufficient for file export use case

## Integration Points

### Dependencies
- **cypcb-world:** Component queries (RefDes, Position, Rotation, FootprintRef)
- **cypcb-core:** Coordinate types (Nm, Point)
- **csv crate:** CSV serialization with serde
- **serde_json:** JSON export
- **chrono:** Timestamp generation for BOM metadata

### Exports
- `group_components()`: Core BOM grouping logic
- `export_bom_csv()`: BOM CSV export
- `export_bom_json()`: BOM JSON export with metadata
- `export_cpl()`: Pick-and-place CSV export
- `CplConfig`: Configuration for machine variations

### Future Integration
- **04-08 ZIP packaging:** Will bundle BOM and CPL files with Gerbers
- **04-09 CLI integration:** Commands like `cypcb export bom` and `cypcb export cpl`
- **UI export dialog:** Select BOM/CPL formats alongside Gerber options

## Test Results

**Total tests:** 30 (18 BOM + 12 CPL)
**Status:** All passing

**BOM Tests:**
- Component grouping: 6 tests
- CSV export: 5 tests
- JSON export: 6 tests
- Natural sorting: 2 tests

**CPL Tests:**
- Export logic: 5 tests
- Coordinate conversion: 2 tests
- Rotation handling: 2 tests
- Configuration: 3 tests

**Overall export crate:** 115 tests passing (30 new + 85 existing)

## Next Phase Readiness

**Blocks:** None

**Enables:**
- 04-08: ZIP packaging (can include BOM/CPL files)
- 04-09: CLI export commands

**Gaps:** None

## Performance Notes

- Component grouping: O(n) with HashMap
- Natural sorting: O(n log n) with extracted keys
- CSV writing: Streaming via csv crate (memory efficient)
- No spatial queries needed (simple iteration)

**Benchmark estimates:**
- 100 components: <1ms
- 1000 components: <10ms
- 10000 components: <100ms

## Technical Debt

None identified. Implementation is straightforward with good test coverage.

## Documentation Needs

**API docs:** Complete with examples in all public functions
**User docs:** Will need:
- BOM format explanation for different manufacturers
- CPL rotation convention guide
- Configuration examples for common machines

## Commit Log

1. **89c3857** - `feat(04-05): implement pick-and-place (CPL) CSV export`
   - Created CPL module with CplEntry and CplConfig
   - CSV export with coordinate/rotation conversion
   - Layer detection and natural sorting
   - 12 tests passing

**Note:** BOM implementation was included in previous commit `34044ff` (mislabeled as 04-04)
- Created BOM module with grouping logic
- CSV and JSON export
- Natural sorting
- 18 tests passing

## Files Created (This Plan)

- `crates/cypcb-export/src/bom/mod.rs` (297 lines) - BOM core types and grouping
- `crates/cypcb-export/src/bom/csv.rs` (229 lines) - BOM CSV export
- `crates/cypcb-export/src/bom/json.rs` (228 lines) - BOM JSON export
- `crates/cypcb-export/src/cpl/mod.rs` (170 lines) - CPL core types
- `crates/cypcb-export/src/cpl/csv.rs` (334 lines) - CPL CSV export

**Total:** 1,258 lines of implementation + tests

## Execution Metrics

- **Duration:** 539 seconds (~9 minutes)
- **Tasks completed:** 2/2
- **Tests added:** 30
- **Commits:** 1 (BOM was in prior commit)
- **Deviations:** 3 auto-fixed bugs (blocking issues)
