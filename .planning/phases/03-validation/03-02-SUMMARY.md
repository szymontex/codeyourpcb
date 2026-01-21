---
phase: 03-validation
plan: 02
subsystem: footprint-library
tags: [footprints, gullwing, soic, sot, qfp, ipc-7351b]

dependency_graph:
  requires:
    - 01-04 (ECS Components - PadShape, Layer types)
    - 01-02 (Core Types - Nm, Point, Rect)
  provides:
    - SOIC-8, SOIC-14 footprints for op-amps, timers
    - SOT-23, SOT-23-5 footprints for transistors, voltage regulators
    - TQFP-32 footprint for microcontrollers
    - gullwing_footprint() parametric generator
  affects:
    - 03-xx (DRC can validate IC-based designs)
    - 04-xx (Gerber export for IC-based boards)

tech_stack:
  added: []
  patterns:
    - Parametric footprint generators for IC packages
    - Counter-clockwise pin numbering (IC standard)
    - IPC-7351B courtyard calculation

key_files:
  created:
    - crates/cypcb-world/src/footprint/gullwing.rs
  modified:
    - crates/cypcb-world/src/footprint/mod.rs
    - crates/cypcb-world/src/footprint/library.rs

decisions:
  - id: FTP-GULLWING-GENERATOR
    choice: "Parametric gullwing_footprint() for dual-row ICs"
    reason: "Reduces duplication, consistent pin positioning"
  - id: FTP-PIN-NUMBERING
    choice: "Counter-clockwise from bottom-left"
    reason: "Standard IC convention, matches KiCad/Altium"
  - id: FTP-COURTYARD
    choice: "Body + 0.5mm margin per IPC-7351B"
    reason: "Industry standard for assembly clearance"

metrics:
  duration: 4m 12s
  completed: 2026-01-21
---

# Phase 03 Plan 02: IC Footprints (SOIC/SOT/QFP) Summary

**One-liner:** Gull-wing IC footprints (SOIC-8/14, SOT-23/5, TQFP-32) with parametric generator following IPC-7351B dimensions

## What Was Built

### gullwing.rs (687 lines)

Created a new footprint module for gull-wing IC packages with:

1. **`gullwing_footprint()` parametric generator:**
   - Generates dual-row IC footprints (SOIC, SSOP, etc.)
   - Parameters: pin_count, pitch, pad_width, pad_length, row_span, body_size
   - Counter-clockwise pin numbering from bottom-left
   - Automatic IPC-7351B courtyard calculation (body + 0.5mm)

2. **SOIC footprints:**
   - `soic8()`: 8-pin, 1.27mm pitch, 5.4mm row span
   - `soic14()`: 14-pin, 1.27mm pitch, 5.4mm row span

3. **SOT footprints (manual layout for asymmetry):**
   - `sot23()`: 3-pin asymmetric (2 bottom, 1 top)
   - `sot23_5()`: 5-pin (3 left, 2 right)

4. **QFP footprints:**
   - `tqfp32()`: 32-pin (8 per side), 0.8mm pitch, 7mm body, 9mm row span

### FootprintLibrary Integration

- Added `register_builtin_gullwing()` method
- All 5 IC footprints accessible via `FootprintLibrary::new().get("SOIC-8")` etc.

## Commits

| Commit | Description |
|--------|-------------|
| 48e8426 | feat(03-02): add gullwing footprint module with SOIC and QFP/SOT |
| 38ee738 | feat(03-02): register gullwing footprints in FootprintLibrary |

## Test Coverage

14 new tests added:
- 10 tests in gullwing.rs (dimensions, pin positions, layers, courtyard)
- 4 tests in library.rs (library registration, accessibility)

Total cypcb-world tests: 148 (94 unit + 54 doc tests)

## Dimensions Reference

| Footprint | Pins | Pitch | Pad Size | Row Span | Body |
|-----------|------|-------|----------|----------|------|
| SOIC-8 | 8 | 1.27mm | 1.5x0.6mm | 5.4mm | 5.0x4.0mm |
| SOIC-14 | 14 | 1.27mm | 1.5x0.6mm | 5.4mm | 5.0x8.7mm |
| SOT-23 | 3 | - | 0.6x1.0mm | 1.9mm | 3.0x2.5mm |
| SOT-23-5 | 5 | 0.95mm | 1.0x0.6mm | 2.4mm | 3.0x3.0mm |
| TQFP-32 | 32 | 0.8mm | 0.45x1.5mm | 9.0mm | 7.0x7.0mm |

## Deviations from Plan

None - plan executed exactly as written.

## Success Criteria Verification

| Criterion | Status |
|-----------|--------|
| SOIC-8 and SOIC-14 footprints with 1.27mm pitch | PASS |
| SOT-23 (3-pin) and SOT-23-5 (5-pin) footprints | PASS |
| TQFP-32 footprint with 0.8mm pitch | PASS |
| All footprints registered in FootprintLibrary | PASS |
| IPC-7351B courtyard calculation (body + 0.5mm margin) | PASS |
| Counter-clockwise pin numbering from bottom-left | PASS |

## Next Phase Readiness

**Blockers:** None

**Ready for:**
- DRC development can now validate designs using microcontrollers and op-amps
- Example boards with ATmega/STM32 (TQFP-32) or NE555/LM358 (SOIC-8)
- SOT-23 transistors and SOT-23-5 voltage regulators in designs
