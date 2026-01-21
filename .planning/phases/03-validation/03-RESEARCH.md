# Phase 3: Validation - Research

**Researched:** 2026-01-21
**Domain:** DRC Engine, Manufacturer Rules, Custom Footprints (QFP/SOIC/SOT)
**Confidence:** HIGH

## Summary

Phase 3 builds a Design Rule Check (DRC) system that validates PCB designs against manufacturer constraints before fabrication. The research confirms that the existing spatial index (rstar R*-tree) is the correct foundation for DRC clearance checking, following the same patterns used by KiCad's DRC engine. The system will validate clearances, trace widths, drill sizes, and net connectivity.

**Key findings:**

1. **DRC Algorithm Pattern:** KiCad's DRC uses a two-phase approach: (1) R*-tree spatial query to find bounding box candidates, (2) detailed shape collision testing with exact clearance values. This pattern maps directly to the existing `SpatialIndex` in cypcb-world.

2. **Manufacturer Presets:** JLCPCB and PCBWay have well-documented design rules. JLCPCB's standard 2-layer rules are: 6mil (0.15mm) min trace/space, 0.3mm min drill (mechanical), 0.2mm min via drill. PCBWay is similar: 3mil capable but 6mil recommended. These will serve as built-in presets.

3. **Non-blocking DRC:** The user decided DRC runs on file save (like ESLint). The Rust engine computes violations, returning results to the renderer which displays markers. Render-first, DRC-after ensures no blocking.

4. **QFP/SOIC/SOT Footprints:** IPC-7351B defines standard land patterns for gull-wing packages. The existing `chip_footprint()` helper pattern can be extended to a `gullwing_footprint()` generator for SOIC/QFP, with proper toe/heel/side fillet calculations.

5. **Custom Footprint Syntax:** The DSL should support inline footprint definitions with numeric pad numbering, following the existing component definition pattern. Courtyards can be auto-calculated per IPC-7351B (body + 0.25mm clearance).

**Primary recommendation:** Create a `cypcb-drc` crate with rule evaluation against the ECS world, using the spatial index for candidate queries. Return structured `DrcViolation` results with source spans for error display. Manufacturer presets are Rust structs, not TOML/JSON, for simplicity.

---

## Standard Stack

The established libraries/tools for this phase:

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rstar | 0.12 | Spatial queries for clearance checking | Already used in cypcb-world, O(log n) queries |
| bevy_ecs | 0.15 | Query engine for DRC checks | Already used, efficient archetype queries |
| miette | 7.6 | DRC error display with source spans | Already used, beautiful error messages |
| thiserror | 2.0 | DRC error type definitions | Already used, standard pattern |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| rayon | 1.10 | Parallel DRC checks | Optional: parallelize large board checks |
| hashbrown | 0.15 | Fast HashSet for checked pairs | Avoid redundant bidirectional checks |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom clearance math | geo crate | geo is overkill for AABB + simple shapes |
| Rust rule presets | TOML/JSON config | Complexity; Rust structs are simpler, type-safe |
| rayon parallelism | Single-threaded | Parallelism optional; 100-component board is fast single-threaded |

**Installation:**

```toml
# crates/cypcb-drc/Cargo.toml
[package]
name = "cypcb-drc"
version = "0.1.0"
edition = "2021"

[dependencies]
cypcb-core = { path = "../cypcb-core" }
cypcb-world = { path = "../cypcb-world" }
cypcb-parser = { path = "../cypcb-parser" }

thiserror = { workspace = true }
miette = { workspace = true }
hashbrown = "0.15"

# Optional parallelism
rayon = { version = "1.10", optional = true }

[features]
default = []
parallel = ["rayon"]
```

---

## Architecture Patterns

### Recommended Project Structure

```
crates/
  cypcb-drc/                    # NEW: Design Rule Check engine
    src/
      lib.rs                    # Public API: run_drc(), DrcResult
      rules/
        mod.rs
        clearance.rs            # Trace-trace, trace-pad clearance
        trace_width.rs          # Minimum trace width validation
        drill_size.rs           # Minimum drill size validation
        connectivity.rs         # Unconnected pin detection
      presets/
        mod.rs
        jlcpcb.rs               # JLCPCB standard rules
        pcbway.rs               # PCBWay standard rules
      violation.rs              # DrcViolation type with source span
      engine.rs                 # DRC engine orchestration
  cypcb-world/
    src/
      footprint/
        smd.rs                  # EXISTING: chip footprints
        tht.rs                  # EXISTING: through-hole footprints
        gullwing.rs             # NEW: SOIC, QFP, SOT footprints
```

### Pattern 1: DRC Rule as Trait

**What:** Each DRC rule is a struct implementing a `DrcRule` trait.

**When to use:** All rule types.

**Example:**

```rust
// crates/cypcb-drc/src/rules/mod.rs

use cypcb_world::BoardWorld;
use crate::violation::DrcViolation;
use crate::presets::DesignRules;

/// A single DRC rule that can be executed against the board.
pub trait DrcRule: Send + Sync {
    /// Rule identifier for error messages.
    fn name(&self) -> &'static str;

    /// Execute the rule check against the board world.
    /// Returns a list of violations (empty if rule passes).
    fn check(
        &self,
        world: &BoardWorld,
        rules: &DesignRules,
    ) -> Vec<DrcViolation>;
}

/// Built-in rules
pub struct ClearanceRule;
pub struct MinTraceWidthRule;
pub struct MinDrillSizeRule;
pub struct UnconnectedPinRule;
```

### Pattern 2: Two-Phase Clearance Checking (KiCad-style)

**What:** Use R*-tree for candidate selection, then exact distance calculation.

**When to use:** All clearance-based rules.

**Example:**

```rust
// crates/cypcb-drc/src/rules/clearance.rs

use cypcb_core::{Nm, Point, Rect};
use cypcb_world::{BoardWorld, SpatialIndex};
use hashbrown::HashSet;

impl DrcRule for ClearanceRule {
    fn name(&self) -> &'static str {
        "clearance"
    }

    fn check(
        &self,
        world: &BoardWorld,
        rules: &DesignRules,
    ) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_clearance = rules.min_clearance;

        // Track checked pairs to avoid A-B and B-A redundancy
        let mut checked_pairs: HashSet<(u32, u32)> = HashSet::new();

        // For each item in spatial index
        let spatial = world.ecs().resource::<SpatialIndex>();

        for entry in spatial.iter() {
            // Expand bounding box by max clearance to find candidates
            let query_region = Rect::new(
                Point::new(
                    Nm(entry.envelope.lower()[0] - min_clearance.0),
                    Nm(entry.envelope.lower()[1] - min_clearance.0),
                ),
                Point::new(
                    Nm(entry.envelope.upper()[0] + min_clearance.0),
                    Nm(entry.envelope.upper()[1] + min_clearance.0),
                ),
            );

            // Phase 1: R*-tree query for candidates
            for candidate in spatial.query_region_entries(query_region.min, query_region.max) {
                if candidate.entity == entry.entity {
                    continue; // Skip self
                }

                // Skip if different layers (no overlap)
                if !entry.layers_overlap(candidate.layer_mask) {
                    continue;
                }

                // Canonical ordering to avoid duplicate checks
                let pair = canonical_pair(entry.entity.index(), candidate.entity.index());
                if !checked_pairs.insert(pair) {
                    continue; // Already checked
                }

                // Phase 2: Exact distance calculation
                // (simplified - actual implementation needs shape-aware distance)
                let distance = aabb_distance(&entry.envelope, &candidate.envelope);

                if distance < min_clearance.0 {
                    violations.push(DrcViolation::clearance(
                        entry.entity,
                        candidate.entity,
                        Nm(distance),
                        min_clearance,
                    ));
                }
            }
        }

        violations
    }
}

fn canonical_pair(a: u32, b: u32) -> (u32, u32) {
    if a < b { (a, b) } else { (b, a) }
}

fn aabb_distance(a: &AABB<[i64; 2]>, b: &AABB<[i64; 2]>) -> i64 {
    // Calculate minimum distance between two AABBs
    let dx = (a.lower()[0].max(b.lower()[0]) - a.upper()[0].min(b.upper()[0])).max(0);
    let dy = (a.lower()[1].max(b.lower()[1]) - a.upper()[1].min(b.upper()[1])).max(0);

    // Euclidean distance (squared root avoided for performance in comparisons)
    // For DRC, we use Manhattan or actual Euclidean based on requirement
    ((dx * dx + dy * dy) as f64).sqrt() as i64
}
```

### Pattern 3: DRC Violation with Source Span

**What:** Violations carry location info for error display and click-to-zoom.

**Example:**

```rust
// crates/cypcb-drc/src/violation.rs

use bevy_ecs::entity::Entity;
use cypcb_core::{Nm, Point};
use cypcb_parser::ast::Span;

/// A design rule violation.
#[derive(Debug, Clone)]
pub struct DrcViolation {
    /// Type of violation.
    pub kind: ViolationKind,
    /// Location on the board (for click-to-zoom).
    pub location: Point,
    /// Primary entity involved.
    pub entity: Entity,
    /// Secondary entity (for clearance violations).
    pub other_entity: Option<Entity>,
    /// Source span in the DSL file (if available).
    pub source_span: Option<Span>,
    /// Human-readable description.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    /// Clearance between two objects is too small.
    Clearance,
    /// Trace width is below minimum.
    TraceWidth,
    /// Drill size is below minimum.
    DrillSize,
    /// Pin has no net connection.
    UnconnectedPin,
    /// Via drill is below minimum.
    ViaDrill,
    /// Annular ring is below minimum.
    AnnularRing,
}

impl DrcViolation {
    pub fn clearance(
        entity: Entity,
        other: Entity,
        actual: Nm,
        required: Nm,
    ) -> Self {
        DrcViolation {
            kind: ViolationKind::Clearance,
            location: Point::ORIGIN, // Computed from entities
            entity,
            other_entity: Some(other),
            source_span: None,
            message: format!(
                "Clearance violation: {} actual, {} required",
                actual.to_mm_string(),
                required.to_mm_string(),
            ),
        }
    }

    pub fn drill_size(entity: Entity, actual: Nm, required: Nm) -> Self {
        DrcViolation {
            kind: ViolationKind::DrillSize,
            location: Point::ORIGIN,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "Drill size violation: {} actual, {} minimum",
                actual.to_mm_string(),
                required.to_mm_string(),
            ),
        }
    }

    pub fn unconnected_pin(entity: Entity, pin: &str, refdes: &str) -> Self {
        DrcViolation {
            kind: ViolationKind::UnconnectedPin,
            location: Point::ORIGIN,
            entity,
            other_entity: None,
            source_span: None,
            message: format!("Unconnected pin: {}.{}", refdes, pin),
        }
    }
}
```

### Pattern 4: Manufacturer Preset Structs

**What:** Design rules as typed Rust structs, not config files.

**Example:**

```rust
// crates/cypcb-drc/src/presets/mod.rs

use cypcb_core::Nm;

/// Complete set of design rules for a board.
#[derive(Debug, Clone)]
pub struct DesignRules {
    /// Minimum clearance between copper features.
    pub min_clearance: Nm,
    /// Minimum trace width.
    pub min_trace_width: Nm,
    /// Minimum drill size (mechanical drilling).
    pub min_drill_size: Nm,
    /// Minimum via drill size.
    pub min_via_drill: Nm,
    /// Minimum annular ring width.
    pub min_annular_ring: Nm,
    /// Minimum silkscreen line width.
    pub min_silk_width: Nm,
    /// Minimum copper to board edge clearance.
    pub min_edge_clearance: Nm,
}

impl DesignRules {
    /// JLCPCB standard 2-layer board rules.
    /// Source: https://jlcpcb.com/capabilities/pcb-capabilities
    pub fn jlcpcb_2layer() -> Self {
        DesignRules {
            min_clearance: Nm::from_mm(0.15),       // 6 mil
            min_trace_width: Nm::from_mm(0.15),     // 6 mil
            min_drill_size: Nm::from_mm(0.3),       // 0.3mm mechanical
            min_via_drill: Nm::from_mm(0.2),        // 0.2mm via
            min_annular_ring: Nm::from_mm(0.15),    // 6 mil
            min_silk_width: Nm::from_mm(0.15),      // 6 mil
            min_edge_clearance: Nm::from_mm(0.3),   // 0.3mm
        }
    }

    /// JLCPCB 4-layer board rules (tighter tolerances available).
    pub fn jlcpcb_4layer() -> Self {
        DesignRules {
            min_clearance: Nm::from_mm(0.1),        // 4 mil
            min_trace_width: Nm::from_mm(0.1),      // 4 mil
            min_drill_size: Nm::from_mm(0.2),       // 0.2mm
            min_via_drill: Nm::from_mm(0.2),
            min_annular_ring: Nm::from_mm(0.125),   // 5 mil
            min_silk_width: Nm::from_mm(0.15),
            min_edge_clearance: Nm::from_mm(0.25),
        }
    }

    /// PCBWay standard rules.
    /// Source: https://www.pcbway.com/capabilities.html
    pub fn pcbway_standard() -> Self {
        DesignRules {
            min_clearance: Nm::from_mm(0.15),       // Recommended 6 mil
            min_trace_width: Nm::from_mm(0.15),
            min_drill_size: Nm::from_mm(0.2),       // Mechanical
            min_via_drill: Nm::from_mm(0.2),
            min_annular_ring: Nm::from_mm(0.15),
            min_silk_width: Nm::from_mm(0.22),      // 8.66 mil
            min_edge_clearance: Nm::from_mm(0.3),
        }
    }

    /// Relaxed rules for prototyping (larger margins).
    pub fn prototype() -> Self {
        DesignRules {
            min_clearance: Nm::from_mm(0.2),        // 8 mil
            min_trace_width: Nm::from_mm(0.25),     // 10 mil
            min_drill_size: Nm::from_mm(0.4),
            min_via_drill: Nm::from_mm(0.3),
            min_annular_ring: Nm::from_mm(0.2),
            min_silk_width: Nm::from_mm(0.2),
            min_edge_clearance: Nm::from_mm(0.5),
        }
    }
}
```

### Pattern 5: Gull-Wing Footprint Generator (SOIC/QFP)

**What:** Parametric generator for IPC-7351B gull-wing packages.

**Example:**

```rust
// crates/cypcb-world/src/footprint/gullwing.rs

use cypcb_core::{Nm, Point, Rect};
use super::library::{Footprint, PadDef};
use crate::components::{Layer, PadShape};

/// Generate a gull-wing IC footprint (SOIC, QFP, etc).
///
/// # Arguments
///
/// * `name` - Footprint identifier (e.g., "SOIC-8")
/// * `description` - Human-readable description
/// * `pin_count` - Total number of pins
/// * `pitch` - Pin pitch (center-to-center)
/// * `pad_width` - Width of each pad
/// * `pad_length` - Length of each pad (toe to heel)
/// * `row_span` - Distance between pad row centers
/// * `body_size` - Component body dimensions (width, height)
///
/// Pins are numbered counter-clockwise starting from bottom-left
/// (standard IC numbering convention).
pub fn gullwing_footprint(
    name: &str,
    description: &str,
    pin_count: usize,
    pitch: Nm,
    pad_width: Nm,
    pad_length: Nm,
    row_span: Nm,
    body_size: (Nm, Nm),
) -> Footprint {
    assert!(pin_count % 2 == 0, "Pin count must be even for dual-row package");

    let pins_per_side = pin_count / 2;
    let half_span = Nm(row_span.0 / 2);
    let smd_layers = vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask];

    let mut pads = Vec::with_capacity(pin_count);

    // Calculate vertical offset for centering
    let total_height = Nm(pitch.0 * (pins_per_side - 1) as i64);
    let y_offset = Nm(total_height.0 / 2);

    // Left side (pins 1 to pins_per_side), bottom to top
    for i in 0..pins_per_side {
        let pin_num = i + 1;
        let y = Nm(i as i64 * pitch.0) - y_offset;

        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(-half_span, y),
            size: (pad_length, pad_width),  // Horizontal pad
            drill: None,
            layers: smd_layers.clone(),
        });
    }

    // Right side (pins pins_per_side+1 to pin_count), top to bottom
    for i in 0..pins_per_side {
        let pin_num = pin_count - i;
        let y = Nm((pins_per_side - 1 - i) as i64 * pitch.0) - y_offset;

        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(half_span, y),
            size: (pad_length, pad_width),
            drill: None,
            layers: smd_layers.clone(),
        });
    }

    // Courtyard: body + 0.25mm clearance per IPC-7351B
    let courtyard_margin = Nm::from_mm(0.5);  // 0.25mm each side

    Footprint {
        name: name.into(),
        description: description.into(),
        pads,
        bounds: Rect::from_center_size(Point::ORIGIN, body_size),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (body_size.0 + courtyard_margin, body_size.1 + courtyard_margin),
        ),
    }
}

/// SOIC-8 (150mil body width, 1.27mm pitch)
pub fn soic8() -> Footprint {
    gullwing_footprint(
        "SOIC-8",
        "Small Outline IC, 8 pins, 1.27mm pitch",
        8,
        Nm::from_mm(1.27),         // pitch
        Nm::from_mm(0.6),          // pad width
        Nm::from_mm(1.5),          // pad length
        Nm::from_mm(5.4),          // row span
        (Nm::from_mm(5.0), Nm::from_mm(4.0)),  // body
    )
}

/// SOIC-14 (150mil body width, 1.27mm pitch)
pub fn soic14() -> Footprint {
    gullwing_footprint(
        "SOIC-14",
        "Small Outline IC, 14 pins, 1.27mm pitch",
        14,
        Nm::from_mm(1.27),
        Nm::from_mm(0.6),
        Nm::from_mm(1.5),
        Nm::from_mm(5.4),
        (Nm::from_mm(5.0), Nm::from_mm(8.7)),
    )
}

/// SOT-23 (3-pin small outline transistor)
pub fn sot23() -> Footprint {
    // SOT-23 is special: 2 pins on one side, 1 on the other
    let smd_layers = vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask];

    Footprint {
        name: "SOT-23".into(),
        description: "Small Outline Transistor, 3 pins".into(),
        pads: vec![
            // Pin 1: left side
            PadDef {
                number: "1".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::from_mm(-0.95), Nm::from_mm(-1.0)),
                size: (Nm::from_mm(0.6), Nm::from_mm(1.0)),
                drill: None,
                layers: smd_layers.clone(),
            },
            // Pin 2: right side bottom
            PadDef {
                number: "2".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::from_mm(0.95), Nm::from_mm(-1.0)),
                size: (Nm::from_mm(0.6), Nm::from_mm(1.0)),
                drill: None,
                layers: smd_layers.clone(),
            },
            // Pin 3: top center
            PadDef {
                number: "3".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::ZERO, Nm::from_mm(1.0)),
                size: (Nm::from_mm(0.6), Nm::from_mm(1.0)),
                drill: None,
                layers: smd_layers,
            },
        ],
        bounds: Rect::from_center_size(
            Point::ORIGIN,
            (Nm::from_mm(3.0), Nm::from_mm(2.5)),
        ),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (Nm::from_mm(3.5), Nm::from_mm(3.0)),
        ),
    }
}

/// SOT-23-5 (5-pin variant)
pub fn sot23_5() -> Footprint {
    let smd_layers = vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask];
    let pitch = Nm::from_mm(0.95);

    Footprint {
        name: "SOT-23-5".into(),
        description: "Small Outline Transistor, 5 pins".into(),
        pads: vec![
            // Pins 1-3: left side, bottom to top
            PadDef {
                number: "1".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::from_mm(-1.2), -pitch),
                size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
                drill: None,
                layers: smd_layers.clone(),
            },
            PadDef {
                number: "2".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::from_mm(-1.2), Nm::ZERO),
                size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
                drill: None,
                layers: smd_layers.clone(),
            },
            PadDef {
                number: "3".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::from_mm(-1.2), pitch),
                size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
                drill: None,
                layers: smd_layers.clone(),
            },
            // Pins 4-5: right side, top to bottom
            PadDef {
                number: "4".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::from_mm(1.2), pitch),
                size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
                drill: None,
                layers: smd_layers.clone(),
            },
            PadDef {
                number: "5".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::from_mm(1.2), -pitch),
                size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
                drill: None,
                layers: smd_layers,
            },
        ],
        bounds: Rect::from_center_size(
            Point::ORIGIN,
            (Nm::from_mm(3.0), Nm::from_mm(3.0)),
        ),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (Nm::from_mm(3.5), Nm::from_mm(3.5)),
        ),
    }
}

/// TQFP-32 (7x7mm body, 0.8mm pitch)
pub fn tqfp32() -> Footprint {
    // QFP: 4-sided gull-wing
    let pin_count = 32;
    let pins_per_side = pin_count / 4;
    let pitch = Nm::from_mm(0.8);
    let pad_width = Nm::from_mm(0.45);
    let pad_length = Nm::from_mm(1.5);
    let body_size = Nm::from_mm(7.0);
    let row_span = Nm::from_mm(9.0);  // Pad center to pad center

    let half_span = Nm(row_span.0 / 2);
    let smd_layers = vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask];

    let mut pads = Vec::with_capacity(pin_count);

    // Calculate offset for centering
    let side_length = Nm(pitch.0 * (pins_per_side - 1) as i64);
    let offset = Nm(side_length.0 / 2);

    let mut pin_num = 1;

    // Bottom side (left to right)
    for i in 0..pins_per_side {
        let x = Nm(i as i64 * pitch.0) - offset;
        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(x, -half_span),
            size: (pad_width, pad_length),
            drill: None,
            layers: smd_layers.clone(),
        });
        pin_num += 1;
    }

    // Right side (bottom to top)
    for i in 0..pins_per_side {
        let y = Nm(i as i64 * pitch.0) - offset;
        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(half_span, y),
            size: (pad_length, pad_width),
            drill: None,
            layers: smd_layers.clone(),
        });
        pin_num += 1;
    }

    // Top side (right to left)
    for i in 0..pins_per_side {
        let x = offset - Nm(i as i64 * pitch.0);
        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(x, half_span),
            size: (pad_width, pad_length),
            drill: None,
            layers: smd_layers.clone(),
        });
        pin_num += 1;
    }

    // Left side (top to bottom)
    for i in 0..pins_per_side {
        let y = offset - Nm(i as i64 * pitch.0);
        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(-half_span, y),
            size: (pad_length, pad_width),
            drill: None,
            layers: smd_layers.clone(),
        });
        pin_num += 1;
    }

    Footprint {
        name: "TQFP-32".into(),
        description: "Thin Quad Flat Package, 32 pins, 0.8mm pitch".into(),
        pads,
        bounds: Rect::from_center_size(Point::ORIGIN, (body_size, body_size)),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (body_size + Nm::from_mm(0.5), body_size + Nm::from_mm(0.5)),
        ),
    }
}
```

### Anti-Patterns to Avoid

- **O(n^2) clearance checking:** Always use spatial index for candidate selection first
- **Floating-point distance comparison:** Use integer nanometers; compare squared distances when possible
- **Blocking DRC:** Always run DRC async/after render; never block file save
- **Config files for simple rules:** Rust structs are simpler; config files add parsing complexity
- **Checking same pair twice:** Use canonical ordering (min, max) to avoid A-B and B-A checks

---

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Spatial candidate query | O(n^2) all-pairs | rstar `query_region_entries()` | Already have R*-tree, O(log n) |
| Manufacturer rules | JSON/TOML parser | Rust `DesignRules` struct | Type-safe, no parsing overhead |
| Error display | Custom formatting | miette with `#[diagnostic]` | Already used, beautiful output |
| Duplicate pair tracking | Vec with contains() | hashbrown HashSet | O(1) lookup |
| IPC-7351B footprint math | Manual dimensions | Parametric generator | Consistency, fewer bugs |

**Key insight:** The hard spatial work is already done. The spatial index from Phase 1 is exactly what KiCad's DRC uses internally. The DRC engine is mostly iteration and comparison logic.

---

## Common Pitfalls

### Pitfall 1: Checking Clearance on Wrong Layers

**What goes wrong:** Report violations between copper on different layers that never touch.

**Why it happens:** Forgetting to filter by layer mask before detailed checking.

**How to avoid:**
1. Always check `entry.layers_overlap(candidate.layer_mask)` before distance calculation
2. Use `query_region_on_layers()` when the active layer is known
3. Test with multi-layer boards having copper on different layers

**Warning signs:** False positives on boards with overlapping top/bottom copper.

### Pitfall 2: Redundant Bidirectional Checks

**What goes wrong:** DRC takes twice as long, reports duplicate violations.

**Why it happens:** Checking A-to-B and then B-to-A.

**How to avoid:**
1. Use canonical pair ordering: `if a < b { (a, b) } else { (b, a) }`
2. Track checked pairs in HashSet
3. Skip when pair already checked

**Warning signs:** Duplicate violations in output, slow DRC on simple boards.

### Pitfall 3: Missing Same-Net Waiver

**What goes wrong:** DRC reports "clearance violation" for connected pads on the same net.

**Why it happens:** Pads on the same net should be allowed to touch/overlap.

**How to avoid:**
1. Look up NetId for both entities before reporting clearance violation
2. If same net, skip clearance check (same net = allowed to connect)
3. Still check for shorts (different net items at zero distance)

**Warning signs:** False positives for every component pad-to-pad connection.

### Pitfall 4: Blocking File Save

**What goes wrong:** User types, saves, application freezes until DRC completes.

**Why it happens:** Running DRC synchronously on save event.

**How to avoid:**
1. File save triggers render immediately
2. DRC runs in background (or after render cycle)
3. Results appear asynchronously (status bar updates, markers appear)
4. Use progress indicator ("Checking...")

**Warning signs:** User reports "lag when saving."

### Pitfall 5: Integer Overflow in Distance Calculation

**What goes wrong:** Large coordinate values cause overflow in distance^2 calculation.

**Why it happens:** Squaring large i64 values can exceed i64 range.

**How to avoid:**
1. Use i128 for squared distance intermediates
2. Or use checked_mul with fallback
3. Test with coordinates near board edges (max nm values)

**Warning signs:** Incorrect distances reported, panics on large boards.

---

## Code Examples

Verified patterns from research:

### DRC Engine API

```rust
// crates/cypcb-drc/src/lib.rs

use cypcb_world::BoardWorld;
use cypcb_parser::ast::SourceFile;

pub mod presets;
pub mod rules;
pub mod violation;

pub use presets::DesignRules;
pub use violation::{DrcViolation, ViolationKind};

/// Result of running DRC on a board.
#[derive(Debug, Clone)]
pub struct DrcResult {
    /// List of violations found.
    pub violations: Vec<DrcViolation>,
    /// Time taken to run DRC (for performance tracking).
    pub duration_ms: u64,
}

impl DrcResult {
    /// Check if the board passed all checks.
    pub fn passed(&self) -> bool {
        self.violations.is_empty()
    }

    /// Number of violations.
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }
}

/// Run DRC on a board world.
///
/// # Arguments
///
/// * `world` - The board world to check
/// * `rules` - Design rules to check against
///
/// # Returns
///
/// DrcResult with all violations found.
pub fn run_drc(world: &BoardWorld, rules: &DesignRules) -> DrcResult {
    use std::time::Instant;

    let start = Instant::now();
    let mut violations = Vec::new();

    // Run each rule checker
    let checkers: Vec<Box<dyn rules::DrcRule>> = vec![
        Box::new(rules::ClearanceRule),
        Box::new(rules::MinTraceWidthRule),
        Box::new(rules::MinDrillSizeRule),
        Box::new(rules::UnconnectedPinRule),
    ];

    for checker in &checkers {
        violations.extend(checker.check(world, rules));
    }

    DrcResult {
        violations,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}
```

### Unconnected Pin Detection

```rust
// crates/cypcb-drc/src/rules/connectivity.rs

use bevy_ecs::prelude::*;
use cypcb_world::{BoardWorld, RefDes, NetConnections, FootprintRef};
use cypcb_world::footprint::FootprintLibrary;

impl DrcRule for UnconnectedPinRule {
    fn name(&self) -> &'static str {
        "unconnected-pin"
    }

    fn check(
        &self,
        world: &BoardWorld,
        _rules: &DesignRules,
    ) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let lib = FootprintLibrary::new();  // Or pass as param

        // Query all components with net connections
        let ecs = world.ecs();
        let mut query = ecs.query::<(Entity, &RefDes, &FootprintRef, &NetConnections)>();

        for (entity, refdes, footprint_ref, nets) in query.iter(ecs) {
            // Get footprint definition
            let Some(footprint) = lib.get(footprint_ref.as_str()) else {
                continue;  // Unknown footprint - skip
            };

            // Check each pad has a net connection
            for pad in &footprint.pads {
                if nets.pin_net(&pad.number).is_none() {
                    violations.push(DrcViolation::unconnected_pin(
                        entity,
                        &pad.number,
                        refdes.as_str(),
                    ));
                }
            }
        }

        violations
    }
}
```

### DSL Syntax for Manufacturer Preset (Claude's Discretion)

```cypcb
version 1

// Specify manufacturer rules (optional - defaults to jlcpcb_2layer)
rules jlcpcb_2layer

// Or with overrides
rules jlcpcb_2layer {
    clearance 0.2mm      // Override default
    trace_width 0.2mm    // Override default
}

board myboard {
    size 50mm x 50mm
    layers 2
}
```

### DSL Syntax for Custom Footprint (Claude's Discretion)

```cypcb
version 1

// Define a custom footprint
footprint MY_SOIC_8 {
    description "Custom SOIC-8 with thermal pad"

    // Pads use numeric IDs
    pad 1 rect at -2.7mm, -1.905mm size 1.5mm x 0.6mm
    pad 2 rect at -2.7mm, -0.635mm size 1.5mm x 0.6mm
    pad 3 rect at -2.7mm, 0.635mm size 1.5mm x 0.6mm
    pad 4 rect at -2.7mm, 1.905mm size 1.5mm x 0.6mm
    pad 5 rect at 2.7mm, 1.905mm size 1.5mm x 0.6mm
    pad 6 rect at 2.7mm, 0.635mm size 1.5mm x 0.6mm
    pad 7 rect at 2.7mm, -0.635mm size 1.5mm x 0.6mm
    pad 8 rect at 2.7mm, -1.905mm size 1.5mm x 0.6mm

    // Optional thermal pad
    pad 9 rect at 0mm, 0mm size 3.0mm x 3.0mm

    // Courtyard auto-calculated if not specified
    // Or manually: courtyard 6mm x 5mm
}

// Use the custom footprint
component U1 ic "MY_SOIC_8" {
    value "ATtiny85"
    at 25mm, 25mm
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| O(n^2) clearance | R*-tree spatial index | Always (KiCad, Altium) | <1s for 1000+ items |
| Config file rules | Type-safe structs | Preference | Compile-time validation |
| Blocking DRC | Async/background DRC | 2020+ (modern EDA) | Non-blocking UX |
| Manual footprint entry | IPC-7351B generators | Industry standard | Consistency, fewer errors |

**Deprecated/outdated:**
- **Floating-point coordinates for DRC**: Integer nm prevents precision issues
- **Synchronous DRC on every change**: Too slow; batch on save is standard

---

## Open Questions

Things that couldn't be fully resolved:

1. **Keepout zone representation**
   - What we know: Need to support copper keepouts for antenna areas, mounting holes
   - What's unclear: How to represent in ECS - separate entity type? Component with special flag?
   - Recommendation: Add `Keepout` component with enum for keepout type (copper, route, via). Query during DRC.

2. **Net class syntax**
   - What we know: Users want per-net rule overrides (power traces wider, high-speed tighter)
   - What's unclear: Exact DSL syntax for defining classes and assigning nets
   - Recommendation: Defer net class implementation to Phase 3.5 or later. Start with board-wide rules.

3. **Courtyard overlap checking**
   - What we know: IPC-7351B defines assembly courtyard for component spacing
   - What's unclear: Should courtyard overlap be a DRC error or warning?
   - Recommendation: Make it an error by default (courtyards shouldn't overlap). Add config option to disable if needed.

---

## Sources

### Primary (HIGH confidence)

- [rstar RTree documentation](https://docs.rs/rstar/latest/rstar/struct.RTree.html) - Query methods for spatial index
- [KiCad DRC copper clearance source](https://docs.kicad.org/doxygen/drc__test__provider__copper__clearance_8cpp_source.html) - Two-phase algorithm pattern
- [JLCPCB Capabilities](https://jlcpcb.com/capabilities/pcb-capabilities) - Manufacturer design rules
- [PCBWay Capabilities](https://www.pcbway.com/capabilities.html) - Manufacturer design rules
- [IPC-7351B Standard Guide](https://www.protoexpress.com/blog/features-of-ipc-7351-standards-to-design-pcb-component-footprint/) - Footprint dimension calculations

### Secondary (MEDIUM confidence)

- [JLCPCB Design Rules Guide](https://www.schemalyzer.com/en/blog/manufacturing/jlcpcb/jlcpcb-design-rules) - Detailed rule specifications
- [IPC-7351 SOIC Specification](https://blog.snapeda.com/2015/07/13/the-ipc-7351-specification-explained-soic-components/) - SOIC pad calculations
- [Altium DRC Documentation](https://www.altium.com/documentation/altium-designer/pcb/drc) - Online/batch DRC patterns
- [EasyEDA DRC Guide](https://docs.easyeda.com/en/PCB/Design-Rule-Check/) - Real-time DRC feedback

### Tertiary (LOW confidence)

- Various forum posts on KiCad/Altium DRC behavior - Implementation edge cases

---

## Metadata

**Confidence breakdown:**
- DRC algorithm: HIGH - KiCad source code confirms R*-tree approach
- Manufacturer rules: HIGH - Official capability pages with exact values
- Footprint dimensions: HIGH - IPC-7351B is industry standard
- DSL syntax for rules/footprints: MEDIUM - Claude's discretion, proposed patterns
- Performance targets: MEDIUM - <1s for 100 components is reasonable estimate

**Research date:** 2026-01-21
**Valid until:** 2026-04-21 (90 days - stable domain, manufacturer specs rarely change)

---

## Recommended Plan Structure

Based on research, Phase 3 should be structured as:

### Task Groups

1. **DRC Crate Setup** (2 tasks)
   - Create cypcb-drc crate with proper dependencies
   - Define DrcRule trait and DrcViolation type

2. **Manufacturer Presets** (2 tasks)
   - Implement DesignRules struct with JLCPCB/PCBWay presets
   - Add DSL syntax for rule selection (optional override)

3. **Clearance Checking** (3 tasks)
   - Implement ClearanceRule using spatial index
   - Handle layer filtering and same-net waivers
   - Test with multi-component boards

4. **Additional Rules** (3 tasks)
   - Minimum trace width validation
   - Minimum drill size validation
   - Unconnected pin detection

5. **DRC Integration** (2 tasks)
   - Wire DRC to hot reload pipeline (run on save)
   - Return violations to renderer

6. **Violation Display** (2 tasks)
   - Render circle markers at violation locations
   - Status bar with error count, click-to-zoom

7. **IC Footprints** (3 tasks)
   - Gull-wing generator (SOIC family)
   - QFP generator
   - SOT family (SOT-23, SOT-23-5)

8. **Custom Footprint Syntax** (2 tasks)
   - DSL syntax for inline footprint definitions
   - Parser support and library registration

### Critical Path

```
DRC Crate Setup
    |
    v
Manufacturer Presets --> Clearance Rule --> Additional Rules
    |                         |                    |
    v                         v                    v
DSL Syntax -----------> DRC Integration --> Violation Display
    |
    v
IC Footprints --> Custom Footprint Syntax
```

### Estimated Scope

- **Total tasks:** 19-21
- **Estimated effort:** Medium (1-2 weeks for experienced Rust developer)
- **Risk areas:** Layer-aware clearance (edge cases), DSL syntax design (lock-in)
