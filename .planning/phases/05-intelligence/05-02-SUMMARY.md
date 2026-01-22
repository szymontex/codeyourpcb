---
phase: 05-intelligence
plan: 02
title: IPC-2221 Trace Width Calculator
subsystem: calculation
tags: [rust, ipc-2221, electrical, trace-width]

dependency-graph:
  requires: [01-02]
  provides: [cypcb-calc crate, TraceWidthCalculator, TraceWidthParams, TraceWidthResult]
  affects: [05-01, 05-03]

tech-stack:
  added: []
  patterns: [builder pattern, warning enum]

key-files:
  created:
    - crates/cypcb-calc/Cargo.toml
    - crates/cypcb-calc/src/lib.rs
    - crates/cypcb-calc/src/trace_width.rs
  modified:
    - Cargo.toml

decisions:
  - id: ipc2221-formula
    choice: I = k * dT^0.44 * A^0.725 formula with k=0.048 (external) and k=0.024 (internal)
    rationale: Standard IPC-2221 formula, widely used in industry
  - id: warning-enum
    choice: TraceWidthWarning enum for out-of-range conditions
    rationale: Clear categorization of accuracy limits and design concerns
  - id: builder-params
    choice: Builder pattern for TraceWidthParams
    rationale: Ergonomic API with sensible defaults and method chaining

metrics:
  tasks: 2/2
  tests: 18 unit + 7 doc
  duration: ~8 minutes
  completed: 2026-01-22
---

# Phase 5 Plan 2: IPC-2221 Trace Width Calculator Summary

**One-liner:** IPC-2221 trace width calculator with external/internal layer constants, copper weight support, and accuracy warnings.

## What Was Built

Created the `cypcb-calc` crate implementing IPC-2221 trace width calculation for determining minimum trace width based on current requirements.

### TraceWidthParams

Input parameters with builder pattern:
- `current_amps: f64` - Required current capacity
- `temp_rise_c: f64` - Allowed temperature rise (default: 10C)
- `copper_oz: f64` - Copper weight in oz/ft² (default: 1.0)
- `is_external: bool` - External vs internal layer (default: external)
- `ambient_temp_c: f64` - Ambient temperature (default: 25C)

Builder methods:
- `new(current_amps)` - Create with required current
- `with_temp_rise(c)` - Set temperature rise
- `with_copper_oz(oz)` - Set copper weight
- `internal()` / `external()` - Set layer position
- `with_ambient_temp(c)` - Set ambient (informational)

### TraceWidthResult

Output with calculated values:
- `width: Nm` - Minimum trace width in nanometers
- `cross_section_mm2: f64` - Cross-sectional area for reference
- `warnings: Vec<TraceWidthWarning>` - Accuracy/design warnings

Helper methods:
- `width_mm()` - Width in millimeters
- `width_mil()` - Width in mils
- `has_warnings()` - Check for warnings

### TraceWidthWarning

Warning conditions for formula accuracy limits:
- `CurrentTooHigh` - >35A (formula accuracy degrades)
- `TempRiseTooLow` - <10C (may require impractical traces)
- `TempRiseTooHigh` - >100C (delamination risk)
- `WidthExceedsMax` - >10mm (consider multiple traces)
- `CopperWeightNonStandard` - Not 0.5/1.0/2.0/3.0 oz

### TraceWidthCalculator

Static calculator methods:
- `calculate(&params) -> TraceWidthResult` - Full calculation with warnings
- `min_width_for_current(amps, is_external) -> Nm` - Quick calculation
- `with_defaults() -> TraceWidthParams` - Get default params for customization

### IPC-2221 Formula

The standard formula: `I = k * dT^0.44 * A^0.725`

Where:
- `I` = current in amperes
- `k` = 0.048 (external) or 0.024 (internal)
- `dT` = temperature rise in Celsius
- `A` = cross-sectional area in mils²

Solved for area: `A = (I / (k * dT^0.44))^(1/0.725)`
Width: `width = A / (copper_oz * 1.378)`

## Test Coverage

18 unit tests covering:
- Reference values: 1A/2A/3A/5A at 10C rise
- Internal vs external layer (internal ~2x wider)
- Temperature rise effect (higher rise = narrower)
- Copper weight effect (thicker copper = narrower)
- All warning conditions
- Builder pattern
- Convenience methods

7 doc tests with examples for:
- Module-level example
- TraceWidthParams builder
- TraceWidthCalculator::calculate
- TraceWidthCalculator::min_width_for_current
- TraceWidthCalculator::with_defaults

## Commits

| Hash | Description |
|------|-------------|
| 8658981 | feat(05-02): create cypcb-calc crate |
| 2d3e6f5 | feat(05-02): implement IPC-2221 trace width calculator |

## Decisions Made

### IPC-2221 Constants
Used standard IPC-2221 k constants:
- k=0.048 for external layers (better heat dissipation via air convection)
- k=0.024 for internal layers (poor heat dissipation through FR4)

This matches published IPC-2221 tables and online calculators.

### Warning Thresholds
Based on IPC-2221 accuracy limits:
- 35A maximum for accurate formula results
- 10-100C temperature rise range
- 10mm maximum before recommending multiple traces
- Standard copper weights for manufacturing availability

### Results as Nm
Return width as `Nm` (nanometers) for consistency with `cypcb-core` types. The result can be converted to mm/mil using existing `Nm` methods.

## Deviations from Plan

None - plan executed exactly as written.

## Technical Notes

### Formula Accuracy
IPC-2221 formulas are approximations accurate for:
- Current up to 35A
- Temperature rise 10-100C
- Trace width up to ~400 mils

Results match IPC-2221 reference tables within 30% tolerance.

### Usage Examples

```rust
use cypcb_calc::{TraceWidthCalculator, TraceWidthParams};

// Simple: 1A on external layer
let width = TraceWidthCalculator::min_width_for_current(1.0, true);
println!("1A external: {:.2}mm", width.to_mm()); // ~0.26mm

// Full parameters
let params = TraceWidthParams::new(5.0)
    .with_temp_rise(20.0)
    .with_copper_oz(2.0)
    .internal();
let result = TraceWidthCalculator::calculate(&params);
println!("Width: {:.2}mm", result.width_mm());
if result.has_warnings() {
    for warning in &result.warnings {
        println!("Warning: {}", warning);
    }
}
```

## Next Phase Readiness

Ready for LSP integration (05-01) where trace width hints can be shown on hover for net constraints.

**Blocking issues:** None
**Dependencies provided:** TraceWidthCalculator for LSP hover hints and router width selection
