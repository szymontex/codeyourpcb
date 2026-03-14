---
id: T01
parent: S04
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
# T01: Export Crate Setup

**# Phase 04 Plan 01: Export Foundation Summary**

## What Happened

# Phase 04 Plan 01: Export Foundation Summary

Created cypcb-export crate with coordinate conversion (nm to Gerber decimal) and aperture management (D-code generation) using integer arithmetic for precision

## What Was Built

### Task 1: Crate Structure

**Deliverables:**
- New `cypcb-export` crate added to workspace
- Dependencies configured: gerber-types 0.7, cypcb-world, cypcb-core, thiserror, serde
- Module structure: `coords` and `apertures` modules
- Re-exports for common types in lib.rs

**Verification:**
- ✓ `cargo check -p cypcb-export` compiles successfully
- ✓ `cargo doc -p cypcb-export --no-deps` generates documentation
- ✓ All module declarations resolve correctly

---

### Task 2: Coordinate Conversion

**Deliverables:**
- `Unit` enum: Millimeters, Inches
- `CoordinateFormat` struct with integer/decimal places
- Constants: `FORMAT_MM_2_6` (2.6mm), `FORMAT_INCH_2_4` (2.4 inch)
- `nm_to_gerber(nm, format)` - Integer arithmetic conversion
- `gerber_format_string(format)` - Generates %FSLAX26Y26*% format strings

**Implementation Highlights:**
```rust
// Integer-only conversion avoiding floating-point precision loss
let integer_part = abs_nm / nm_per_unit;
let remainder = abs_nm % nm_per_unit;
let scale = 10i64.pow(decimal_places as u32);
let fractional_part = (remainder * scale) / nm_per_unit;
```

**Key Features:**
- Handles negative coordinates correctly
- Zero-pads fractional part to match decimal places
- Supports both mm (1mm = 1,000,000 nm) and inches (1" = 25,400,000 nm)
- Deterministic output (no floating-point rounding)

**Test Coverage:**
- ✓ Zero coordinate (0nm → "0.000000")
- ✓ One millimeter (1,000,000nm → "1.000000")
- ✓ Fractional values (1,500,000nm → "1.500000")
- ✓ Negative coordinates (-1,000,000nm → "-1.000000")
- ✓ Inches format (25,400,000nm → "1.0000")
- ✓ Precision edge cases (1nm → "0.000001")
- ✓ Large values (100mm)
- ✓ Format string generation (%FSLAX26Y26*%)
- 11 unit tests passing

---

### Task 3: Aperture Management

**Deliverables:**
- `ApertureShape` enum: Circle, Rectangle, Oblong, RoundRect
- `ApertureManager` struct with D-code assignment
- `get_or_create(shape)` - Automatic shape deduplication
- `to_definitions(format)` - Generates %ADD...% statements
- `aperture_for_pad(pad)` - Maps PadDef to ApertureShape

**Implementation Highlights:**
- D-codes start at 10 (D01-D03 reserved per Gerber spec)
- HashMap-based deduplication: same shape returns same D-code
- Generates standard Gerber aperture definitions:
  - Circle: `%ADD10C,1.000000*%`
  - Rectangle: `%ADD11R,1.000000X0.500000*%`
  - Oblong: `%ADD12O,1.500000X0.800000*%`
  - RoundRect: Falls back to Rectangle with G04 comment

**Shape Mapping:**
```rust
PadShape::Circle       → ApertureShape::Circle { diameter }
PadShape::Rect         → ApertureShape::Rectangle { width, height }
PadShape::Oblong       → ApertureShape::Oblong { width, height }
PadShape::RoundRect{r} → ApertureShape::RoundRect { width, height, corner_ratio }
```

**Test Coverage:**
- ✓ Manager initialization (next_dcode = 10)
- ✓ D-code assignment (10, 11, 12...)
- ✓ Shape reuse (same shape → same D-code)
- ✓ Different shapes get unique D-codes
- ✓ Circle definition generation
- ✓ Rectangle definition generation
- ✓ Oblong definition generation
- ✓ RoundRect fallback with comment
- ✓ Multiple aperture sorted output
- ✓ All pad shape conversions
- 13 unit tests passing

---

## Success Criteria Met

- ✓ cypcb-export crate added to workspace
- ✓ Coordinate conversion produces correct Gerber decimal format
- ✓ Aperture manager generates valid D-code definitions
- ✓ All tests pass (24 unit tests + 7 doc tests = 31 total)
- ✓ Documentation generates successfully
- ✓ Crate compiles without errors

## Deviations from Plan

None - plan executed exactly as written.

## Technical Notes

### Precision Handling

The coordinate conversion uses **integer arithmetic only** to avoid floating-point precision issues:

1. Separate magnitude from sign
2. Integer division for whole part
3. Scaled remainder for fractional part
4. Zero-pad to match decimal places

This ensures deterministic, repeatable output across platforms and compilers.

### RoundRect Limitation

Standard Gerber X2 has no built-in RoundRect aperture type. Current implementation falls back to Rectangle with a G04 comment noting the corner ratio. Future enhancement could implement polygon approximation for true rounded corners.

### D-code Range

Gerber D-codes:
- D01: Draw line
- D02: Move
- D03: Flash
- D10+: User-defined apertures

Starting at D10 follows industry convention and avoids conflicts with reserved codes.

## Commits

- `e21ae88`: feat(04-01): create cypcb-export crate structure
- `5e88de3`: feat(04-01): implement coordinate conversion
- `e86cc18`: feat(04-01): implement aperture management

## Next Steps

**Phase 04 Plan 02: Gerber Layer Export**
- Use coordinate conversion for X/Y values
- Use aperture manager for pad definitions
- Generate complete Gerber files per layer
- Add X2 attributes for layer identification

**Phase 04 Plan 03: Excellon Drill Export**
- Reuse coordinate conversion for drill positions
- Generate tool definitions
- Export drill file with header

## Dependencies for Future Plans

This plan provides:
- `nm_to_gerber()` - Used by all coordinate output
- `CoordinateFormat` - Shared format specification
- `ApertureManager` - Used by all layer exports
- `aperture_for_pad()` - Used by copper layer export
