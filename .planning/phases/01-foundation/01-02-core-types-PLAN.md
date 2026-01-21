---
phase: 01-foundation
plan: 02
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/cypcb-core/src/lib.rs
  - crates/cypcb-core/src/coords.rs
  - crates/cypcb-core/src/units.rs
  - crates/cypcb-core/src/geometry.rs
autonomous: true

must_haves:
  truths:
    - "Coordinates are i64 nanometers internally"
    - "Unit conversions are accurate (mm, mil, inch to nm)"
    - "Geometry types support intersection and containment"
  artifacts:
    - path: "crates/cypcb-core/src/coords.rs"
      provides: "Nm newtype with arithmetic ops"
      exports: ["Nm", "Point"]
    - path: "crates/cypcb-core/src/units.rs"
      provides: "Unit enum and parsing"
      exports: ["Unit"]
    - path: "crates/cypcb-core/src/geometry.rs"
      provides: "Rect, basic geometry"
      exports: ["Rect"]
  key_links:
    - from: "crates/cypcb-core/src/lib.rs"
      to: "coords.rs, units.rs, geometry.rs"
      via: "pub mod re-exports"
      pattern: "pub use coords::"
---

<objective>
Implement the foundational coordinate and geometry types using i64 nanometers.

Purpose: Provide type-safe, precision-correct primitives that prevent floating-point accumulation errors throughout the codebase.

Output: Complete cypcb-core crate with Nm, Point, Rect, and Unit types.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/01-foundation/01-RESEARCH.md

Key research findings:
- Use i64 for nanometer coordinates (KiCad uses i32, we use i64 for headroom)
- Conversion constants: 1mm = 1,000,000nm, 1mil = 25,400nm, 1inch = 25,400,000nm
- Origin: bottom-left, Y-up (mathematical convention)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Implement Nm coordinate type</name>
  <files>crates/cypcb-core/src/coords.rs</files>
  <action>
Create Nm newtype wrapping i64 with:
- ZERO, MAX constants
- from_mm(f64), from_mil(f64), from_inch(f64) constructors
- to_mm(), to_mil(), to_inch() converters
- Implement: Add, Sub, Mul<i64>, Div<i64>, Neg
- Implement: Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord
- Derive serde::Serialize, serde::Deserialize

Create Point struct with x: Nm, y: Nm:
- new(x: Nm, y: Nm) constructor
- from_mm(x: f64, y: f64) convenience constructor
- distance_squared(other: Point) -> i128 (avoid sqrt, use i128 for intermediate)
- Implement same traits as Nm

Include comprehensive doc comments explaining nanometer precision choice.
  </action>
  <verify>
`cargo test -p cypcb-core` passes
Test: Nm::from_mm(1.0).0 == 1_000_000
Test: Point::from_mm(1.0, 2.0).x == Nm(1_000_000)
  </verify>
  <done>Nm and Point types compile with all required traits</done>
</task>

<task type="auto">
  <name>Task 2: Implement Unit enum and geometry types</name>
  <files>
    crates/cypcb-core/src/units.rs
    crates/cypcb-core/src/geometry.rs
    crates/cypcb-core/src/lib.rs
  </files>
  <action>
Create Unit enum in units.rs:
- Variants: Mm, Mil, Inch, Nm
- to_nm(value: f64) -> Nm method
- from_str() for parsing "mm", "mil", "in", "nm"
- Derive: Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize

Create Rect in geometry.rs:
- min: Point, max: Point fields
- new(min, max), from_points(p1, p2) (normalizes min/max)
- width() -> Nm, height() -> Nm
- center() -> Point
- contains(Point) -> bool
- intersects(&Rect) -> bool
- union(&Rect) -> Rect
- Derive same traits as Point

Update lib.rs to:
- Declare modules: coords, units, geometry
- Re-export all public types: pub use coords::*; etc.
- Add crate-level documentation

Add unit tests for:
- Unit conversion accuracy (round-trip mm -> nm -> mm within epsilon)
- Rect intersection logic
- Rect containment logic
  </action>
  <verify>
`cargo test -p cypcb-core` all tests pass
`cargo doc -p cypcb-core --open` shows documentation
  </verify>
  <done>All core types implemented with tests passing</done>
</task>

</tasks>

<verification>
- `cargo test -p cypcb-core` passes all tests
- Unit conversions are accurate:
  - 1mm = 1,000,000nm
  - 1mil = 25,400nm
  - 1inch = 25,400,000nm
- Rect operations are correct (test with known geometries)
- No floating-point types in coordinate storage
</verification>

<success_criteria>
1. Nm type stores i64 internally
2. All arithmetic operations implemented
3. Unit conversions are accurate to nm precision
4. Rect intersection/containment works correctly
5. All types derive Serialize/Deserialize for JSON output
</success_criteria>

<output>
After completion, create `.planning/phases/01-foundation/01-02-SUMMARY.md`
</output>
