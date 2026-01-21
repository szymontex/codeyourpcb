---
phase: 01-foundation
plan: 07
type: summary
completed: 2026-01-21
duration: ~15 minutes

subsystem: footprint-library
tags: [footprint, smd, tht, ipc-7351, pad, library]

dependency-graph:
  requires: ["01-02", "01-04"]
  provides: ["FootprintLibrary", "Footprint", "PadDef"]
  affects: ["01-08", "02-*"]

tech-stack:
  added: []
  patterns: ["Factory functions for footprint generation"]

key-files:
  created:
    - crates/cypcb-world/src/footprint/mod.rs
    - crates/cypcb-world/src/footprint/library.rs
    - crates/cypcb-world/src/footprint/smd.rs
    - crates/cypcb-world/src/footprint/tht.rs
  modified:
    - crates/cypcb-core/src/geometry.rs
    - crates/cypcb-world/src/lib.rs

decisions:
  - id: FP-01
    choice: "IPC-7351B nominal density for pad dimensions"
    reason: "Industry standard, good balance between assembly yield and density"
  - id: FP-02
    choice: "Factory functions over struct literals for footprints"
    reason: "Ensures consistent pad layer assignments and courtyard calculations"
  - id: FP-03
    choice: "Footprint names match common convention (0402, DIP-8, etc.)"
    reason: "Familiar to PCB designers, matches datasheet and library naming"

metrics:
  tests-added: 16
  tests-total-crate: 43
  doc-tests-added: 9
  lines-added: ~922
---

# Phase 01 Plan 07: Footprints Summary

FootprintLibrary with IPC-7351B compliant SMD (0402-2512) and THT (axial, DIP-8, header) footprints.

## What Was Built

### Footprint Data Structures

Created a comprehensive footprint system with three core types:

1. **`PadDef`** - Individual pad within a footprint
   - Number/name (e.g., "1", "A1")
   - Shape (Circle, Rect, RoundRect, Oblong)
   - Position relative to footprint origin
   - Size in nanometers (width, height)
   - Optional drill diameter (None for SMD)
   - Layer list (TopCopper, TopPaste, etc.)

2. **`Footprint`** - Complete footprint definition
   - Name and description
   - Vector of PadDef
   - Bounds rectangle (component body)
   - Courtyard rectangle (assembly clearance)

3. **`FootprintLibrary`** - Registry with lookup
   - HashMap-based storage
   - Pre-populated with built-in footprints
   - `get()`, `register()`, `iter()` methods
   - Contains 8 built-in footprints on construction

### SMD Footprints (smd.rs)

Implemented 5 chip resistor/capacitor footprints per IPC-7351B:

| Package | Body Size | Pad Size | Pad Span |
|---------|-----------|----------|----------|
| 0402 | 1.0x0.5mm | 0.6x0.5mm | 1.0mm |
| 0603 | 1.6x0.8mm | 0.9x0.95mm | 1.6mm |
| 0805 | 2.0x1.25mm | 1.0x1.45mm | 1.9mm |
| 1206 | 3.2x1.6mm | 1.15x1.8mm | 3.4mm |
| 2512 | 6.3x3.2mm | 1.4x3.4mm | 6.5mm |

All SMD pads include: TopCopper, TopPaste, TopMask layers.

### THT Footprints (tht.rs)

Implemented 3 through-hole footprints:

| Package | Lead Spacing | Drill | Pad Size |
|---------|--------------|-------|----------|
| AXIAL-300 | 300mil (7.62mm) | 0.8mm | 1.6mm circle |
| DIP-8 | 300mil rows, 100mil pitch | 0.8mm | 1.6mm oblong |
| PIN-HDR-1x2 | 100mil (2.54mm) | 1.0mm | 1.7mm |

THT pads include: TopCopper, BottomCopper layers.

### Geometry Enhancement

Added `Rect::from_center_size()` to cypcb-core for easier footprint bounds creation.

## Technical Decisions

### IPC-7351B Nominal Density

Chose "nominal" land pattern density from IPC-7351B, which provides good manufacturing yield while maintaining reasonable component density. The alternative "least" density is for hand soldering; "most" is for ultra-dense boards.

### Layer Assignments

- SMD pads: TopCopper + TopPaste + TopMask (standard reflow soldering)
- THT pads: TopCopper + BottomCopper (plated through-hole)
- Courtyard: Body + 0.25mm clearance per IPC-7351B

### Factory Pattern

Used factory functions (`chip_0402()`, `dip8()`, etc.) rather than exposing constructors. This ensures:
- Consistent layer assignments
- Proper courtyard calculations
- Correct pad positioning

## Files Changed

| File | Change |
|------|--------|
| `crates/cypcb-core/src/geometry.rs` | Added `Rect::from_center_size()` helper |
| `crates/cypcb-world/src/lib.rs` | Added `pub mod footprint` |
| `crates/cypcb-world/src/footprint/mod.rs` | Module definition and exports |
| `crates/cypcb-world/src/footprint/library.rs` | PadDef, Footprint, FootprintLibrary |
| `crates/cypcb-world/src/footprint/smd.rs` | SMD footprint generators |
| `crates/cypcb-world/src/footprint/tht.rs` | THT footprint generators |

## Tests Added

### Unit Tests (16)

**library.rs (5):**
- `test_library_has_builtin_footprints` - All 8 built-ins present
- `test_footprint_lookup` - get() returns correct footprint
- `test_custom_footprint_registration` - register() adds new footprints
- `test_pad_def_is_smd` - is_smd()/is_through_hole() methods
- `test_footprint_get_pad` - get_pad() lookup by number

**smd.rs (5):**
- `test_chip_0402_dimensions` - Correct pad sizes
- `test_smd_pads_symmetric` - Pads symmetric about Y axis
- `test_smd_pads_have_correct_layers` - TopCopper, TopPaste, TopMask
- `test_all_smd_footprints_have_two_pads` - All 5 have 2 pads
- `test_pad_span_increases_with_package_size` - Larger packages have wider spans

**tht.rs (6):**
- `test_axial_300mil_dimensions` - 300mil spacing, 0.8mm drill
- `test_dip8_has_8_pins` - 8 pads with drills
- `test_dip8_row_spacing` - 300mil row spacing
- `test_pin_header_1x2` - 100mil pitch, square pin 1
- `test_tht_pads_have_correct_layers` - TopCopper, BottomCopper

### Doc Tests (9)

All examples in module and function docs are verified.

## Verification

All success criteria met:

- [x] 0402, 0603, 0805, 1206, 2512 SMD footprints available
- [x] Axial, DIP-8, pin header through-hole footprints available
- [x] Pad sizes match IPC standards
- [x] All dimensions in integer nanometers (Nm type)
- [x] Library lookup by name works

## Deviations from Plan

None - plan executed exactly as written.

## Next Steps

The footprint library is ready for use by:
- **01-08 (AST Sync)**: Map parsed component footprint references to library
- **Phase 2 (Rendering)**: Render pads with correct shapes and sizes

Future enhancements (not in scope):
- More footprint types (QFP, BGA, SOT-23)
- Bottom-side placement (flip layers)
- Custom footprint definition in DSL
- KiCad footprint import
