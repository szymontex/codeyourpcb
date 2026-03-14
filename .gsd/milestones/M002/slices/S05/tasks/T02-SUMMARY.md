---
id: T02
parent: S05
milestone: M002
provides:
  - PhysicalUnit enum with 23 variants across 7 quantity categories (resistance, capacitance, inductance, voltage, current, frequency, power)
  - PhysicalQuantity enum for quantity-level operations
  - FromStr, Display, to_base_f64, from_base_f64, suffix(), quantity() on PhysicalUnit
  - PhysicalValue AST node now uses typed PhysicalUnit instead of raw String
  - Parser convert_physical_value resolves unit strings to typed PhysicalUnit with error reporting
key_files:
  - crates/cypcb-core/src/physical_units.rs
  - crates/cypcb-core/src/lib.rs
  - crates/cypcb-parser/src/ast.rs
  - crates/cypcb-parser/src/parser.rs
key_decisions:
  - PhysicalUnit lives in cypcb-core as a separate module from units.rs — keeps length units (nm-based) and electrical units (SI-based) cleanly separated
  - PhysicalValue.unit changed from String to PhysicalUnit directly in the AST — parser already depends on cypcb-core so no new dependency needed
  - Unit suffix matching is case-sensitive to match the grammar exactly (kohm not kOhm, Mohm not MOHM)
patterns_established:
  - Physical unit resolution happens in convert_physical_value during CST→AST conversion, producing typed PhysicalUnit with ParseError on invalid units
  - Tolerance absolute/range values also resolve to typed PhysicalUnit (not just the main value)
observability_surfaces:
  - ParseError::InvalidPhysicalUnit carries span info for diagnostics — surfaces in LSP error messages
duration: 20min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: Add PhysicalUnit system to cypcb-core and wire value property

**Added typed PhysicalUnit enum (23 variants, 7 quantities) to cypcb-core and wired parser's PhysicalValue AST to use it — all conversions proven correct across 49 core tests and 80 parser tests.**

## What Happened

Created `crates/cypcb-core/src/physical_units.rs` with `PhysicalQuantity` (7 categories) and `PhysicalUnit` (23 variants) enums. Each unit has: `quantity()` for category, `suffix()` matching grammar exactly, `to_base_f64()`/`from_base_f64()` for SI normalization, `FromStr` for case-sensitive parsing, and `Display` for round-trip fidelity.

Changed `PhysicalValue.unit` in `ast.rs` from `String` to `PhysicalUnit`. Updated `convert_physical_value` in `parser.rs` to resolve unit strings via `FromStr`, emitting `ParseError::InvalidPhysicalUnit` on failure. Also updated tolerance conversion paths (absolute and range) to resolve their unit strings to typed `PhysicalUnit`.

Updated 6 existing parser tests that compared `pv.unit` against string literals to use `PhysicalUnit::*` variants.

## Verification

- `cargo test --manifest-path crates/cypcb-core/Cargo.toml` — 49 tests + 30 doctests pass (including 17 new PhysicalUnit tests)
- `cargo test --manifest-path crates/cypcb-parser/Cargo.toml` — 80 tests + 4 doctests pass (including updated typed unit assertions)
- `cargo build -p cypcb-core -p cypcb-parser -p cypcb-world -p cypcb-lsp -p cypcb-rules` — all compile clean
- `cargo build --target wasm32-unknown-unknown -p cypcb-render` — WASM build succeeds
- Backward compat test (`test_backward_compat_all_example_files`) passes — all 10 v1 files parse with zero errors

### Slice-level verification status (intermediate task):
- ✅ `cargo test --manifest-path crates/cypcb-parser/Cargo.toml` — all pass
- ✅ `cargo test --manifest-path crates/cypcb-core/Cargo.toml` — all pass
- ⚠️ `cargo build` — core crates compile; GTK system deps prevent full Tauri build (pre-existing)
- ✅ `cargo build --target wasm32-unknown-unknown -p cypcb-render` — passes
- ✅ Backward compat: all 10 v1 files parse with zero errors
- ⏳ Forward test: v2 example files — deferred to T03

## Diagnostics

- `cargo test -p cypcb-core -- physical_units` — run all PhysicalUnit tests
- `cargo test -p cypcb-parser -- test_parse_physical_value` — run physical value parsing tests
- `ParseError::InvalidPhysicalUnit` includes span + source for LSP diagnostics

## Deviations

None — followed the plan. The existing `InvalidPhysicalUnit` error variant from T01 was already in place.

## Known Issues

None.

## Files Created/Modified

- `crates/cypcb-core/src/physical_units.rs` — new file: PhysicalQuantity, PhysicalUnit enums with conversion/parsing/display logic and 17 tests
- `crates/cypcb-core/src/lib.rs` — added `pub mod physical_units` and re-exports
- `crates/cypcb-parser/src/ast.rs` — PhysicalValue.unit changed from String to PhysicalUnit
- `crates/cypcb-parser/src/parser.rs` — convert_physical_value and tolerance paths now resolve to typed PhysicalUnit; 6 test assertions updated
