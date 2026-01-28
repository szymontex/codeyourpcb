---
phase: 04-export
plan: 02
subsystem: export
status: complete
completed: 2026-01-28
duration: 35min
tags: [gerber, x2, manufacturing, copper, soldermask, solderpaste]

requires:
  - 04-01  # Export foundation (coords, apertures)

provides:
  - gerber_x2_export
  - copper_layer_export
  - mask_paste_export

affects:
  - 04-03  # Drill export will use similar patterns
  - 04-07  # Gerber job file references these layers

tech-stack:
  added:
    - chrono (0.4)        # ISO8601 timestamps for X2 attributes
    - bevy_ecs (workspace) # ECS queries for component iteration
  patterns:
    - x2_attributes       # TF.FileFunction, TF.GenerationSoftware
    - polarity_control    # %LPD*% for dark polarity
    - aperture_reuse      # Hash-based deduplication across layers

key-files:
  created:
    - crates/cypcb-export/src/gerber/mod.rs
    - crates/cypcb-export/src/gerber/header.rs
    - crates/cypcb-export/src/gerber/copper.rs
    - crates/cypcb-export/src/gerber/mask.rs
  modified:
    - crates/cypcb-export/src/lib.rs
    - crates/cypcb-export/Cargo.toml
    - Cargo.toml (workspace)

decisions:
  - title: "Gerber X2 format with attributes"
    rationale: "Modern CAM systems expect X2 metadata for layer identification and traceability"
    alternatives: ["RS-274D legacy format"]

  - title: "Dark polarity for mask/paste"
    rationale: "Standard approach - positive areas indicate exposed copper/paste"
    tradeoffs: "Must remember polarity when viewing files"

  - title: "0.05mm default mask expansion"
    rationale: "Common manufacturing tolerance for solder mask openings"
    configurable: true

  - title: "Integer arithmetic for aperture sizing"
    rationale: "Maintains precision when applying expansion/reduction"
    impact: "Consistent with nm-based coordinate system"

  - title: "Exclude THT pads from paste"
    rationale: "Through-hole components don't use solder paste"
    implementation: "Check for pad.drill.is_some()"

  - title: "Component rotation via trigonometry"
    rationale: "Accurate pad positioning for rotated components"
    complexity: "Small floating-point error acceptable (<0.1µm)"
---

# Phase 04 Plan 02: Copper Layer Gerber Export Summary

**One-liner:** Complete Gerber X2 export for copper, soldermask, and solderpaste layers with aperture management and position calculations.

## What Was Built

Implemented full Gerber file export for PCB manufacturing:

### 1. Gerber X2 Header Module
- **GerberFileFunction** enum for all layer types (Copper, Mask, Paste, Silk, Profile)
- **write_header()** generates complete X2-compliant headers
- X2 attributes: TF.GenerationSoftware, TF.CreationDate, TF.FileFunction, TF.Part
- CopperSide and Side enums for layer designation
- Automatic layer numbering (L1 for top, Ln for bottom based on total layers)

### 2. Copper Layer Export
- **export_copper_layer()** main export function
- Pad export with component position/rotation handling
- Trace export with D01/D02 draw/move commands
- Via export as flashed circles
- **calculate_pad_position()** handles rotation via trigonometry
- **via_spans_layer()** determines which layers vias appear on
- ExportError type for footprint lookup failures

### 3. Soldermask Layer Export
- **export_soldermask()** generates mask opening files
- Aperture expansion (default 0.05mm) for manufacturing clearance
- %LPD*% dark polarity (positive = exposed areas)
- All pads receive mask openings (SMD and THT)

### 4. Solderpaste Layer Export
- **export_solderpaste()** generates paste stencil files
- SMD-only filtering (excludes THT pads with drills)
- Optional paste reduction for fine-pitch components
- %LPD*% dark polarity

### 5. Configuration System
- **MaskPasteConfig** struct with builder pattern
- Configurable mask expansion (Nm)
- Configurable paste reduction (0.0 to 1.0)
- Sensible defaults (0.05mm expansion, 0% reduction)

## Verification Results

**Test Coverage:** 29 new tests, all passing

### Header Tests (12 tests)
- X2 attribute generation for all layer types
- Copper layer numbering (L1, L2, L4 for 4-layer boards)
- MM and inch format support
- Board name inclusion
- ISO8601 timestamp generation

### Copper Tests (8 tests)
- Empty layer export (header + footer only)
- Single pad export with position
- Trace export with multiple segments
- Via export
- Component rotation (90° test with <0.1µm error)
- Bottom layer number calculation (L4 for 4-layer)
- Through-hole via layer spanning

### Mask Tests (9 tests)
- Empty soldermask export
- Empty solderpaste export
- Mask expansion (1.0mm → 1.1mm with 0.05mm expansion)
- Paste reduction (1.0mm → 0.9mm with 10% reduction)
- THT pad exclusion from paste
- Config builder pattern
- Dark polarity presence

## Technical Highlights

### Aperture Reuse
All exports use the shared `ApertureManager` for deduplication:
- Pads, traces, vias all generate apertures
- Hash-based deduplication prevents duplicate D-codes
- Definitions emitted once per file

### Rotation Handling
Component rotation uses standard rotation matrix:
```rust
let angle_rad = (rotation_millideg as f64) / 1000.0 * PI / 180.0;
let rotated_x = pad_x * cos(theta) - pad_y * sin(theta);
let rotated_y = pad_x * sin(theta) + pad_y * cos(theta);
```
Floating-point error is <0.1µm, acceptable for PCB manufacturing.

### Integer Arithmetic for Sizing
Expansion/reduction use integer ops where possible:
```rust
// Expansion: add to both sides
diameter: diameter + (expansion.0 * 2)

// Reduction: multiply then cast
diameter: ((diameter as f64) * (1.0 - reduction)) as i64
```

### Gerber Command Structure
All exports follow standard structure:
1. Header (G04 comments, %FS, %MO, X2 attributes)
2. Aperture definitions (%ADD)
3. Polarity declaration (%LP)
4. Drawing commands (D01/D02/D03)
5. End of file (M02*)

## Deviations from Plan

None - plan executed exactly as written.

All planned features implemented:
- [x] X2 header with all attributes
- [x] Copper layer export (pads, traces, vias)
- [x] Soldermask export
- [x] Solderpaste export
- [x] Configuration system
- [x] Comprehensive tests

## Next Phase Readiness

**Phase 4 Export - Next Steps:**

04-03: Excellon drill file export
- Similar pattern to copper export
- Query THT pads and vias
- Generate drill commands with tool definitions
- Reuse coordinate conversion

04-04: Board outline export
- Query board size from BoardWorld
- Generate Profile layer Gerber
- Simple rectangle for MVP

04-07: Gerber job file
- References all exported layers
- JSON format specification
- Layer stackup metadata

**Blockers:** None

**Risks:**
- Excellon format less familiar than Gerber (mitigated by research in 04-RESEARCH.md)
- Board outline may need zone support for complex shapes (defer to future)

**Confidence:** High - core Gerber export patterns proven, extensions straightforward

## Commits

| Task | Commit | Files | Tests |
|------|--------|-------|-------|
| 1. Gerber header module | 7743884 | gerber/header.rs, mod.rs, lib.rs | 12 |
| 2. Copper layer export | e35d257 | gerber/copper.rs | 8 |
| 3. Mask/paste export | 1dbaccc | gerber/mask.rs | 9 |

**Total:** 3 commits, 4 new files, 1,379 lines added, 29 tests

## Performance Notes

**Export Speed:** Not yet measured (no performance tests)
**File Sizes:** Typical outputs:
- Copper layer: ~5-50KB depending on trace count
- Mask/paste: ~1-10KB (fewer features than copper)

**Memory:** Negligible - strings built incrementally, no large allocations

## Integration Points

**Upstream Dependencies:**
- cypcb-world: BoardWorld queries, FootprintLibrary, Layer enum
- cypcb-core: Nm, Point, Rect types
- bevy_ecs: Query system for component iteration

**Downstream Consumers:**
- 04-09: Export CLI will call these functions
- Future: Web UI export button
- Future: CI/CD automated exports

## Known Limitations

1. **Blind/buried vias:** Simplified layer span logic, assumes through-hole
2. **RoundRect apertures:** Fall back to Rectangle with G04 comment (standard Gerber limitation)
3. **Board name:** Currently hardcoded as "board", should extract from BoardWorld
4. **Rotation precision:** <0.1µm error from floating-point trig

All limitations acceptable for MVP, documented for future enhancement.

## Future Enhancements

1. **Polygon fills** for RoundRect apertures (Gerber polygon mode)
2. **Negative plane layers** for power/ground fills
3. **Advanced paste reduction** with per-pad override
4. **Custom X2 attributes** (user-defined metadata)
5. **Panelization** (TF.Part=Array support)

## Learning/Insights

**Gerber Format:** Surprisingly simple once X2 attributes understood
- Core is just coordinate + command (D01/D02/D03)
- Apertures define "what" to draw, commands define "where"
- Polarity control is critical for mask layers

**ECS Queries:** bevy_ecs ergonomics excellent
- `query::<(&Position, &Rotation, &FootprintRef)>()` is clean
- Need `ecs_mut()` for query creation, `ecs()` for iteration

**Trigonometry:** Rotation math straightforward
- Standard 2D rotation matrix works perfectly
- Millidegree storage prevents precision loss
- Floating-point for trig, cast back to i64 for storage

**Builder Pattern:** Effective for configuration
- `with_mask_expansion()` chains nicely
- Defaults in `impl Default` centralize policy
