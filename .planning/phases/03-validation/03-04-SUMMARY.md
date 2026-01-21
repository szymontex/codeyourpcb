---
phase: "03"
plan: "04"
subsystem: parser
tags: [dsl, footprint, tree-sitter, ast]
dependency-graph:
  requires: ["01-05", "01-08"]
  provides: ["custom-footprint-dsl", "footprint-ast", "footprint-sync"]
  affects: ["03-05", "04-01"]
tech-stack:
  added: []
  patterns: ["custom-footprint-definition", "pad-definition-syntax"]
key-files:
  created: []
  modified:
    - crates/cypcb-parser/grammar/grammar.js
    - crates/cypcb-parser/src/ast.rs
    - crates/cypcb-parser/src/parser.rs
    - crates/cypcb-parser/src/lib.rs
    - crates/cypcb-world/src/sync.rs
    - crates/cypcb-world/src/footprint/library.rs
decisions:
  - id: negative-dimensions
    choice: "Support negative dimensions with optional '-' sign in grammar"
    rationale: "Footprint pads often have negative offsets from origin"
  - id: clone-library
    choice: "Clone FootprintLibrary for custom registration"
    rationale: "Non-breaking change, allows custom footprints without mutable reference"
  - id: tht-layer-default
    choice: "THT pads default to TopCopper + BottomCopper layers"
    rationale: "Through-hole naturally spans both copper layers"
  - id: smd-layer-default
    choice: "SMD pads default to TopCopper + TopPaste + TopMask"
    rationale: "Standard SMD pad stack for reflow soldering"
metrics:
  duration: "30 minutes"
  completed: "2026-01-21"
---

# Phase 03 Plan 04: Custom Footprint DSL Summary

**One-liner:** Extends grammar and parser to support inline footprint definitions with pad geometry, enabling custom packages in .cypcb files.

## What Was Built

### 1. Grammar Extension (Task 1)
Extended Tree-sitter grammar with footprint_definition, pad_definition, and related rules:

```javascript
// footprint NAME { ... }
footprint_definition: $ => seq(
  'footprint',
  field('name', $.identifier),
  '{',
  repeat($.footprint_property),
  '}',
),

// pad N shape at X, Y size W x H [drill D]
pad_definition: $ => seq(
  'pad',
  field('number', $.number),
  field('shape', $.pad_shape),
  'at',
  field('x', $.dimension),
  ',',
  field('y', $.dimension),
  'size',
  field('width', $.dimension),
  'x',
  field('height', $.dimension),
  optional(field('drill', $.drill_spec)),
),

pad_shape: $ => choice('rect', 'circle', 'roundrect', 'oblong'),
```

Also added support for negative dimensions (`-1mm`, `-3.81mm`) needed for pad offsets from footprint origin.

### 2. AST Types (Task 2)
Added three new types to `cypcb-parser/src/ast.rs`:

```rust
pub enum PadShape {
    Rect,
    Circle,
    RoundRect,
    Oblong,
}

pub struct PadDef {
    pub number: u32,
    pub shape: PadShape,
    pub x: Dimension,
    pub y: Dimension,
    pub width: Dimension,
    pub height: Dimension,
    pub drill: Option<Dimension>,
    pub span: Span,
}

pub struct FootprintDef {
    pub name: Identifier,
    pub description: Option<String>,
    pub pads: Vec<PadDef>,
    pub courtyard: Option<(Dimension, Dimension)>,
    pub span: Span,
}
```

Extended `Definition` enum with `Footprint(FootprintDef)` variant.

### 3. Parser Implementation (Task 3)
Added CST-to-AST conversion in `parser.rs`:

- `convert_footprint_definition()`: Extracts name, description, pads, courtyard
- `convert_pad_definition()`: Extracts pad number, shape, position, size, drill
- Updated dimension parsing to handle negative signs
- Added footprint_definition case to definition parsing

### 4. Custom Footprint Registration (Task 4)
Updated `sync_ast_to_world()` to register custom footprints:

```rust
// Phase 0: Register custom footprints BEFORE component sync
for def in &ast.definitions {
    if let Definition::Footprint(fp_def) = def {
        let footprint = convert_footprint_def(fp_def);
        lib.register(footprint);
    }
}
```

Conversion functions:
- `convert_footprint_def()`: Converts AST FootprintDef to library Footprint
- `convert_pad_shape()`: Maps AST PadShape to ECS PadShape
- `calculate_footprint_bounds()`: Computes bounding box from pads
- Applies IPC-7351B 0.5mm courtyard margin if not explicit

## Example Usage

```cypcb
version 1

footprint MY_SOIC8 {
    description "Custom SOIC-8 with wider pads"
    pad 1 rect at -2.7mm, -1.905mm size 1.5mm x 0.6mm
    pad 2 rect at -2.7mm, -0.635mm size 1.5mm x 0.6mm
    pad 3 rect at -2.7mm, 0.635mm size 1.5mm x 0.6mm
    pad 4 rect at -2.7mm, 1.905mm size 1.5mm x 0.6mm
    pad 5 rect at 2.7mm, 1.905mm size 1.5mm x 0.6mm
    pad 6 rect at 2.7mm, 0.635mm size 1.5mm x 0.6mm
    pad 7 rect at 2.7mm, -0.635mm size 1.5mm x 0.6mm
    pad 8 rect at 2.7mm, -1.905mm size 1.5mm x 0.6mm
    courtyard 6mm x 5mm
}

component U1 ic "MY_SOIC8" {
    at 15mm, 15mm
}
```

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | Prior commits | Grammar rules already in 0180a14 |
| 2 | 0fd87d5 | FootprintDef and PadDef AST types |
| 3 | 2c47319 | Footprint parsing implementation |
| 4 | 42ac17d | Custom footprint sync registration |

## Tests Added

- `test_parse_footprint_definition`: Basic footprint with description, pads, courtyard
- `test_parse_footprint_with_drill`: THT footprint with drill specifications
- `test_parse_footprint_all_pad_shapes`: All pad shapes (rect, circle, roundrect, oblong)
- `test_pad_shape_parse`: AST PadShape::from_str
- `test_custom_footprint_registration`: End-to-end custom footprint in sync
- `test_custom_footprint_with_tht_pads`: THT footprint registration

## Deviations from Plan

### [Rule 1 - Bug] Negative dimension support
- **Found during:** Task 4 testing
- **Issue:** Grammar didn't support negative dimensions (`-1mm`)
- **Fix:** Added optional `sign` field to dimension rule, updated parser to negate value
- **Commit:** 42ac17d

## Files Changed

| File | Lines Changed | Purpose |
|------|---------------|---------|
| grammar/grammar.js | +53 | footprint grammar rules, negative dimensions |
| src/ast.rs | +79 | FootprintDef, PadDef, PadShape types |
| src/parser.rs | +234 | Footprint/pad parsing, negative handling |
| src/lib.rs | +52 | Tree-sitter grammar tests |
| sync.rs | +98 | Custom footprint registration |
| library.rs | +1 | Clone derive for FootprintLibrary |

## Verification

- [x] Tree-sitter parses footprint syntax without errors
- [x] AST correctly represents footprint, pads, courtyard
- [x] Custom footprints available during component sync
- [x] THT pads get TopCopper + BottomCopper layers
- [x] SMD pads get TopCopper + TopPaste + TopMask layers
- [x] All 211 tests pass (48 parser + 106 world + 57 doctests)

## Dependencies

**Requires:**
- 01-05: Tree-sitter grammar foundation
- 01-08: AST-to-ECS sync infrastructure

**Enables:**
- 03-05: Zone DSL (similar pattern)
- 04-01: DRC checks on custom footprints
