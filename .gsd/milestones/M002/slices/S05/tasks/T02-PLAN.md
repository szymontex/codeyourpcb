---
estimated_steps: 5
estimated_files: 4
---

# T02: Add PhysicalUnit system to cypcb-core and wire value property

**Slice:** S05 — DSL v2 — Modules, Units & Constraints
**Milestone:** M002

## Description

Add a typed `PhysicalUnit` enum to cypcb-core that represents electrical/physical quantities (resistance, capacitance, inductance, voltage, current, frequency, power). This is separate from the existing `Unit` enum (which represents length units with nanometer conversion) because electrical units have fundamentally different base conversions.

Wire the parser's `PhysicalValue` AST node to use `PhysicalUnit` for proper typing, and ensure the `value` component property can accept typed physical values alongside string literals.

## Steps

1. **Add `PhysicalUnit` enum to cypcb-core.** Create `crates/cypcb-core/src/physical_units.rs` with:
   - `PhysicalQuantity` enum: Resistance, Capacitance, Inductance, Voltage, Current, Frequency, Power
   - `PhysicalUnit` enum variants: Ohm/KiloOhm/MegaOhm, PicoFarad/NanoFarad/MicroFarad/MilliFarad, NanoHenry/MicroHenry/MilliHenry/Henry, MilliVolt/Volt/KiloVolt, MicroAmp/MilliAmp/Amp, Hertz/KiloHertz/MegaHertz/GigaHertz, MilliWatt/Watt
   - `quantity(&self) -> PhysicalQuantity` method
   - `to_base_f64(value: f64) -> f64` — normalize to base unit (ohms, farads, henries, volts, amps, hertz, watts)
   - `from_base_f64(value: f64) -> f64` — convert from base unit
   - `suffix(&self) -> &str` — the DSL suffix string
   - `FromStr` implementation mapping suffix strings to variants
   - `Display` implementation

2. **Export from crate root.** Add `pub mod physical_units;` to `crates/cypcb-core/src/lib.rs` and re-export key types.

3. **Update parser AST to use typed PhysicalUnit.** Change `PhysicalValue` in `ast.rs` from using a raw `String` unit to `cypcb_core::PhysicalUnit` (or keep string in AST and resolve in a later phase — depends on whether we want the parser to have a core dependency). Decision: keep the AST using string unit representation since the parser crate already depends on cypcb-core. Add `PhysicalUnit` resolution in the convert step.

4. **Write comprehensive unit tests:** Test `FromStr` for every unit suffix, test `to_base_f64` normalization (e.g., `10kohm` → `10000.0` ohms, `100nF` → `1e-7` farads), test `Display` round-trip, test invalid unit suffix errors.

5. **Verify integration:** `cargo test` in both cypcb-core and cypcb-parser crates. Ensure physical values parsed from grammar map to correct `PhysicalUnit` variants.

## Must-Haves

- [ ] `PhysicalUnit` enum covers all 7 quantity categories with correct variants
- [ ] `FromStr` maps all DSL suffixes to correct variants (ohm, kohm, Mohm, pF, nF, uF, mF, nH, uH, mH, H, mV, V, kV, uA, mA, A, Hz, kHz, MHz, GHz, mW, W)
- [ ] `to_base_f64` normalizes correctly (unit tests prove conversions)
- [ ] Existing `Unit` enum (length units) unchanged — no breaking changes
- [ ] Physical values parsed from grammar resolve to typed `PhysicalUnit`

## Verification

- `cargo test --manifest-path crates/cypcb-core/Cargo.toml` — all tests pass including new PhysicalUnit tests
- `cargo test --manifest-path crates/cypcb-parser/Cargo.toml` — physical value parsing tests pass with typed units

## Inputs

- `crates/cypcb-core/src/units.rs` — existing `Unit` enum (length units, 280 lines) — do NOT modify
- `crates/cypcb-parser/src/ast.rs` — PhysicalValue AST node from T01
- `crates/cypcb-parser/src/parser.rs` — convert_physical_value from T01
- S05-RESEARCH.md — physical unit categories and suffix definitions

## Expected Output

- `crates/cypcb-core/src/physical_units.rs` — new file with `PhysicalUnit`, `PhysicalQuantity` enums + conversion logic
- `crates/cypcb-core/src/lib.rs` — updated with new module export
- `crates/cypcb-parser/src/ast.rs` — PhysicalValue now uses typed unit (not raw string)
- `crates/cypcb-parser/src/parser.rs` — convert_physical_value resolves unit strings to PhysicalUnit
