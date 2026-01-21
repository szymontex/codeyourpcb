---
phase: 01-foundation
plan: 07
type: execute
wave: 3
depends_on: ["01-02", "01-04"]
files_modified:
  - crates/cypcb-world/src/footprint/mod.rs
  - crates/cypcb-world/src/footprint/library.rs
  - crates/cypcb-world/src/footprint/smd.rs
  - crates/cypcb-world/src/footprint/tht.rs
  - crates/cypcb-world/src/lib.rs
autonomous: true

must_haves:
  truths:
    - "Basic SMD footprints (0402-2512) are available"
    - "Basic through-hole footprints are available"
    - "Footprints define pads with correct sizes in nanometers"
  artifacts:
    - path: "crates/cypcb-world/src/footprint/library.rs"
      provides: "FootprintLibrary with lookup"
      exports: ["FootprintLibrary", "Footprint"]
    - path: "crates/cypcb-world/src/footprint/smd.rs"
      provides: "SMD footprint generators"
      min_lines: 50
    - path: "crates/cypcb-world/src/footprint/tht.rs"
      provides: "Through-hole footprint generators"
      min_lines: 30
  key_links:
    - from: "crates/cypcb-world/src/footprint/smd.rs"
      to: "cypcb_core::Nm"
      via: "pad dimensions"
      pattern: "Nm::from_mm"
---

<objective>
Implement the footprint library with basic SMD and through-hole footprints.

Purpose: Provide a registry of standard footprints that can be referenced by component definitions, with accurate pad sizes per IPC standards.

Output: FootprintLibrary with 0402-2512 SMD and basic through-hole footprints.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/01-foundation/01-RESEARCH.md

Requirements covered:
- FTP-01: Basic SMD footprints (0402-2512)
- FTP-02: Basic through-hole footprints

Standard SMD package dimensions (IPC-7351):
- 0402: 1.0mm x 0.5mm body, 0.5mm x 0.5mm pads, 1.1mm span
- 0603: 1.6mm x 0.8mm body, 0.9mm x 0.8mm pads, 1.6mm span
- 0805: 2.0mm x 1.25mm body, 1.0mm x 1.25mm pads, 1.9mm span
- 1206: 3.2mm x 1.6mm body, 1.0mm x 1.6mm pads, 3.4mm span
- 2512: 6.3mm x 3.2mm body, 1.2mm x 3.2mm pads, 6.5mm span
</context>

<tasks>

<task type="auto">
  <name>Task 1: Define Footprint data structures</name>
  <files>
    crates/cypcb-world/src/footprint/mod.rs
    crates/cypcb-world/src/footprint/library.rs
  </files>
  <action>
Create footprint/mod.rs declaring submodules.

Create footprint/library.rs with:
```rust
use std::collections::HashMap;
use cypcb_core::{Nm, Point, Rect};
use crate::components::{PadShape, Layer};

/// A single pad in a footprint
#[derive(Debug, Clone)]
pub struct PadDef {
    pub number: String,        // "1", "2", "A1", etc.
    pub shape: PadShape,
    pub position: Point,       // Relative to footprint origin
    pub size: (Nm, Nm),        // Width x Height
    pub drill: Option<Nm>,     // None for SMD, Some for THT
    pub layers: Vec<Layer>,    // Which copper layers
}

/// Complete footprint definition
#[derive(Debug, Clone)]
pub struct Footprint {
    pub name: String,          // "0402", "DIP-8", etc.
    pub description: String,
    pub pads: Vec<PadDef>,
    pub bounds: Rect,          // Bounding box
    pub courtyard: Rect,       // Assembly courtyard
}

/// Library of known footprints
#[derive(Debug, Default)]
pub struct FootprintLibrary {
    footprints: HashMap<String, Footprint>,
}

impl FootprintLibrary {
    pub fn new() -> Self {
        let mut lib = Self::default();
        lib.register_builtin_smd();
        lib.register_builtin_tht();
        lib
    }

    pub fn get(&self, name: &str) -> Option<&Footprint> {
        self.footprints.get(name)
    }

    pub fn register(&mut self, footprint: Footprint) {
        self.footprints.insert(footprint.name.clone(), footprint);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Footprint)> {
        self.footprints.iter().map(|(k, v)| (k.as_str(), v))
    }

    fn register_builtin_smd(&mut self) { /* filled by smd.rs */ }
    fn register_builtin_tht(&mut self) { /* filled by tht.rs */ }
}
```
  </action>
  <verify>
`cargo build -p cypcb-world` compiles
FootprintLibrary can be instantiated
  </verify>
  <done>Footprint data structures defined</done>
</task>

<task type="auto">
  <name>Task 2: Implement SMD and THT footprints</name>
  <files>
    crates/cypcb-world/src/footprint/smd.rs
    crates/cypcb-world/src/footprint/tht.rs
    crates/cypcb-world/src/footprint/library.rs
    crates/cypcb-world/src/lib.rs
  </files>
  <action>
Create smd.rs with functions that return Footprint:
```rust
use cypcb_core::{Nm, Point, Rect};
use super::library::{Footprint, PadDef};
use crate::components::{PadShape, Layer};

/// Generate standard 2-pad chip resistor/capacitor footprints
pub fn chip_0402() -> Footprint {
    // IPC-7351B nominal: 0.6mm x 0.5mm pads, 1.0mm span
    let pad_w = Nm::from_mm(0.6);
    let pad_h = Nm::from_mm(0.5);
    let span = Nm::from_mm(1.0);  // Center-to-center

    Footprint {
        name: "0402".into(),
        description: "Chip 0402 (1005 metric)".into(),
        pads: vec![
            PadDef {
                number: "1".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm(0) - span / 2, Nm(0)),
                size: (pad_w, pad_h),
                drill: None,
                layers: vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask],
            },
            PadDef {
                number: "2".into(),
                shape: PadShape::Rect,
                position: Point::new(span / 2, Nm(0)),
                size: (pad_w, pad_h),
                drill: None,
                layers: vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask],
            },
        ],
        bounds: Rect::from_center_size(Point::default(), (Nm::from_mm(1.6), Nm::from_mm(0.8))),
        courtyard: Rect::from_center_size(Point::default(), (Nm::from_mm(2.0), Nm::from_mm(1.2))),
    }
}

// Similar for: chip_0603(), chip_0805(), chip_1206(), chip_2512()
```

Create tht.rs with through-hole footprints:
```rust
/// 2-pin through-hole (axial resistor, etc.)
pub fn axial_300mil() -> Footprint {
    // 300mil (7.62mm) lead spacing, 1.0mm drill, 1.8mm pad
    ...
}

/// DIP-8 through-hole IC
pub fn dip8() -> Footprint {
    // 300mil row spacing, 100mil pin pitch
    ...
}

/// 1x2 pin header
pub fn pin_header_1x2() -> Footprint {
    // 100mil (2.54mm) pitch
    ...
}
```

Update library.rs register_builtin_* methods to call these functions.

Update lib.rs to export footprint module.

Add Rect::from_center_size helper to cypcb-core/geometry.rs if not already present.
  </action>
  <verify>
`cargo test -p cypcb-world` passes
FootprintLibrary::new().get("0402") returns valid footprint
All pads have correct nanometer dimensions
  </verify>
  <done>SMD and THT footprints implemented</done>
</task>

</tasks>

<verification>
- FootprintLibrary contains all basic footprints
- Pad dimensions match IPC-7351 standards (within tolerance)
- SMD pads have no drill, THT pads have drill
- All coordinates are in nanometers
- Footprints can be looked up by name
</verification>

<success_criteria>
1. 0402, 0603, 0805, 1206, 2512 SMD footprints available
2. Axial, DIP-8, pin header through-hole footprints available
3. Pad sizes match IPC standards
4. All dimensions in integer nanometers
5. Library lookup by name works
</success_criteria>

<output>
After completion, create `.planning/phases/01-foundation/01-07-SUMMARY.md`
</output>
