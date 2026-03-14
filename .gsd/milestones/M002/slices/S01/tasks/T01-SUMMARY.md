---
id: T01
parent: S01
milestone: M002
provides:
  - cypcb-rules crate with DesignConstraints, SignalClass, Stackup, RoutingRuleSet
  - 35-field DesignConstraints with JLCPCB 2-layer defaults
  - Object-safe RoutingRuleSet trait for A* autorouter integration
key_files:
  - crates/cypcb-rules/src/constraints.rs
  - crates/cypcb-rules/src/signal_class.rs
  - crates/cypcb-rules/src/stackup.rs
  - crates/cypcb-rules/src/routing_rules.rs
  - crates/cypcb-rules/src/lib.rs
  - crates/cypcb-rules/Cargo.toml
key_decisions:
  - Used integer-scaled fields (x100, x1000, x10) for non-dimension values (impedance, dielectric constant, copper weight) to avoid floats while maintaining precision
  - Used u8 layer indices in RoutingRuleSet and SignalClassConstraints to avoid cypcb-world dependency
  - Added serde derives to all public types for serialization roundtrip support
patterns_established:
  - Integer scaling convention for non-dimension physical values (x10 for oz, x100 for ohms/mA, x1000 for εr)
  - Factory method pattern for common stackup configurations
  - SignalClass::ALL const array for exhaustive iteration
observability_surfaces:
  - Compile-time errors if API contracts break
  - cargo test -p cypcb-rules for full verification
  - cargo doc -p cypcb-rules for API inspection
duration: 1 step
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Scaffold cypcb-rules crate with core types, signal classes, and RoutingRuleSet trait

**Built complete `cypcb-rules` crate with 35-field DesignConstraints, 5-variant SignalClass with per-class constraint mappings, 2/4/6-layer Stackup factories, and object-safe RoutingRuleSet trait — 32 unit tests + 5 doc tests pass.**

## What Happened

Created `cypcb-rules` as a leaf crate depending only on `cypcb-core` (+ `serde`). Four modules:

- **constraints.rs**: `DesignConstraints` with 35 fields across 5 categories (basic geometry, advanced geometry, signal integrity, thermal, manufacturing). Default impl uses JLCPCB 2-layer standard values. Non-dimension physical values use integer scaling (copper weight ×10, impedance ×100, dielectric constant ×1000) to avoid floats.

- **signal_class.rs**: `SignalClass` enum (Digital, HighSpeed, Analog, Power, Differential) with `default_constraints()` returning `SignalClassConstraints` per class. Power gets wider traces, HighSpeed/Differential require impedance control and length matching, Analog gets guard trace clearance.

- **stackup.rs**: `Stackup`, `LayerStackEntry`, `LayerType` types with factory methods `two_layer_1oz()`, `four_layer_standard()`, `six_layer_standard()`. Each stackup has correct layer ordering (silk/mask/copper/dielectric) with realistic thicknesses summing to ~1.6mm.

- **routing_rules.rs**: `RoutingRuleSet` trait with 5 methods. Object-safety proven by `&dyn RoutingRuleSet` compilation and dyn-dispatch tests.

## Verification

- `cargo test -p cypcb-rules` — 32 unit tests + 5 doc tests pass
- `cargo clippy -p cypcb-rules -- -D warnings` — no warnings from cypcb-rules (pre-existing clippy warning in cypcb-core is unrelated)
- `cargo build --workspace` — full workspace compiles
- `grep -rn 'cypcb_world\|cypcb_drc' crates/cypcb-rules/` — no results (leaf dependency verified)

### Slice-level checks (partial — T01 is not final task):
- ✅ `cargo test -p cypcb-rules` passes
- ⬜ `cargo test -p cypcb-drc` — not touched in T01, expected to pass from prior state
- ✅ `cargo build --workspace` passes
- ⬜ `docs/pcb-knowledge/` — not in T01 scope
- ✅ `cargo clippy -p cypcb-rules -- -D warnings` — clean

## Diagnostics

- `cargo doc -p cypcb-rules --open` for full API surface
- `cargo test -p cypcb-rules -- --nocapture` for test output
- Pure data types — no runtime behavior, failures are compile-time

## Deviations

- Added `serde` as a runtime dependency and `serde_json` as a dev dependency — not in original plan but necessary for serde derives and roundtrip tests. Both are already workspace dependencies.
- Used integer scaling conventions (x100, x1000, x10) for non-dimension physical values instead of raw floats — aligns with the crate's integer-precision philosophy.

## Known Issues

- Pre-existing clippy warning in `cypcb-core/src/units.rs` (derivable_impls) — not introduced by this task.

## Files Created/Modified

- `crates/cypcb-rules/Cargo.toml` — new crate manifest with cypcb-core + serde dependencies
- `crates/cypcb-rules/src/lib.rs` — crate root with module declarations and re-exports
- `crates/cypcb-rules/src/constraints.rs` — DesignConstraints struct (35 fields) with JLCPCB defaults
- `crates/cypcb-rules/src/signal_class.rs` — SignalClass enum and SignalClassConstraints
- `crates/cypcb-rules/src/stackup.rs` — Stackup, LayerStackEntry, LayerType with 2/4/6-layer factories
- `crates/cypcb-rules/src/routing_rules.rs` — RoutingRuleSet trait definition
- `Cargo.toml` — added cypcb-rules to workspace dependencies
