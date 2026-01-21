---
phase: 03-validation
plan: 03
subsystem: drc-presets
tags: [drc, design-rules, jlcpcb, pcbway, manufacturer-presets]

dependency_graph:
  requires:
    - 03-01 (DRC Crate Setup - DrcRule trait, violation types)
    - 01-02 (Core Types - Nm for dimension values)
  provides:
    - DesignRules struct with 7 manufacturer constraints
    - JLCPCB 2-layer/4-layer presets
    - PCBWay standard preset
    - Prototype preset for relaxed tolerances
    - Preset enum for name-based lookup
  affects:
    - 03-xx (DRC rules use these constraints for validation)
    - DSL parsing (rules keyword can lookup presets by name)

tech_stack:
  added: []
  patterns:
    - Rust structs for manufacturer presets (no config files)
    - Enum with from_name() for DSL integration

key_files:
  created:
    - crates/cypcb-drc/src/presets/mod.rs
    - crates/cypcb-drc/src/presets/jlcpcb.rs
    - crates/cypcb-drc/src/presets/pcbway.rs
  modified:
    - crates/cypcb-drc/src/lib.rs
    - crates/cypcb-drc/src/rules/mod.rs

decisions:
  - id: DRC-RUST-PRESETS
    choice: "Rust structs for manufacturer presets, not TOML/JSON"
    reason: "Type-safe, no parsing overhead, compile-time validation"
  - id: DRC-PRESET-ENUM
    choice: "Preset enum with from_name() method"
    reason: "DSL integration via string lookup while maintaining type safety"
  - id: DRC-DEFAULT-JLCPCB
    choice: "Default preset is JLCPCB 2-layer"
    reason: "Most common hobbyist manufacturer, well-documented specs"

metrics:
  duration: 3m 45s
  completed: 2026-01-21
---

# Phase 03 Plan 03: Manufacturer Presets Summary

**One-liner:** Type-safe manufacturer design rules (JLCPCB 2/4-layer, PCBWay, Prototype) with Preset enum for DSL lookup

## What Was Built

### presets/mod.rs - DesignRules struct and Preset enum

Created a complete design rules configuration system:

1. **DesignRules struct with 7 constraint fields:**
   - `min_clearance` - Minimum copper-to-copper clearance
   - `min_trace_width` - Minimum trace width
   - `min_drill_size` - Minimum mechanical drill diameter
   - `min_via_drill` - Minimum via drill diameter
   - `min_annular_ring` - Minimum copper ring around drill holes
   - `min_silk_width` - Minimum silkscreen line width
   - `min_edge_clearance` - Minimum copper-to-board-edge distance

2. **Preset enum with 4 variants:**
   - `Jlcpcb2Layer` - Standard 2-layer (6 mil, 0.3mm drill)
   - `Jlcpcb4Layer` - 4-layer with tighter tolerances (4 mil, 0.2mm drill)
   - `PcbwayStandard` - PCBWay recommended values
   - `Prototype` - Relaxed rules for hand assembly (8/10 mil)

3. **Name-based lookup for DSL:**
   - `Preset::from_name("jlcpcb")` returns `Some(Preset::Jlcpcb2Layer)`
   - Aliases: "jlcpcb" = "jlcpcb_2layer", "pcbway" = "pcbway_standard"

### presets/jlcpcb.rs - JLCPCB specifications

Implements JLCPCB's documented capabilities:
- 2-layer: 6 mil (0.15mm) clearance/trace, 0.3mm mechanical drill
- 4-layer: 4 mil (0.1mm) clearance/trace, 0.2mm drill, 5 mil annular ring

### presets/pcbway.rs - PCBWay and Prototype specifications

Implements PCBWay's recommended minimums:
- Standard: 6 mil clearance, 0.2mm drill, wider silkscreen (0.22mm)
- Prototype: 8 mil clearance, 10 mil trace, larger margins for reliability

## Commits

| Commit | Description |
|--------|-------------|
| 0180a14 | feat(03-03): implement manufacturer preset design rules |

## Test Coverage

35 total unit tests, 23 preset-specific:
- 13 tests in presets/mod.rs (enum lookup, defaults, comparisons)
- 6 tests in presets/jlcpcb.rs (JLCPCB 2-layer and 4-layer values)
- 4 tests in presets/pcbway.rs (PCBWay and Prototype values)

16 doc tests (all passing), including examples for each preset constructor.

## Manufacturer Rules Reference

| Preset | Clearance | Trace | Drill | Via | Annular | Silk | Edge |
|--------|-----------|-------|-------|-----|---------|------|------|
| JLCPCB 2L | 0.15mm | 0.15mm | 0.3mm | 0.2mm | 0.15mm | 0.15mm | 0.3mm |
| JLCPCB 4L | 0.10mm | 0.10mm | 0.2mm | 0.2mm | 0.125mm | 0.15mm | 0.25mm |
| PCBWay | 0.15mm | 0.15mm | 0.2mm | 0.2mm | 0.15mm | 0.22mm | 0.3mm |
| Prototype | 0.20mm | 0.25mm | 0.4mm | 0.3mm | 0.20mm | 0.20mm | 0.5mm |

## Deviations from Plan

None - plan executed exactly as written.

## Success Criteria Verification

| Criterion | Status |
|-----------|--------|
| DesignRules struct with all manufacturer constraints | PASS |
| JLCPCB 2-layer: 0.15mm clearance, 0.3mm drill, 0.2mm via | PASS |
| JLCPCB 4-layer: 0.1mm clearance, 0.2mm drill | PASS |
| PCBWay standard: 0.15mm clearance, 0.2mm drill | PASS |
| Prototype: 0.2mm clearance (relaxed) | PASS |
| Preset enum with from_name() lookup | PASS |

## Next Phase Readiness

**Blockers:** None

**Ready for:**
- DRC rule implementations can now check against these constraints
- DSL can support `rules jlcpcb` or `rules pcbway_standard` syntax
- Users can create custom DesignRules by constructing directly
