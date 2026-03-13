---
estimated_steps: 5
estimated_files: 7
---

# T02: Manufacturer presets, IPC clearance tables, and preset implementations

**Slice:** S01 — PCB Knowledge Base & Design Rules
**Milestone:** M002

## Description

Implement all manufacturer presets with verified specifications and source URLs in code comments. Each preset produces a complete `DesignConstraints` + `Stackup` pair. Encode IPC-2221 voltage-based clearance tables as lookup functions. Implement `PresetRuleSet` struct that wraps a preset and implements the `RoutingRuleSet` trait — this is the primary way S02's autorouter will consume design rules.

Presets: JLCPCB standard 2L/4L, JLCPCB advanced 2L/4L, PCBWay standard, OSHPark 2L/4L, generic IPC Class 1/2/3. Each has manufacturer-verified values with spec page URLs in comments.

## Steps

1. Create `crates/cypcb-rules/src/presets/mod.rs` with `RulesPreset` enum (all manufacturer variants) and `from_name()` string lookup with aliases. Create `constraints()` and `stackup()` methods on `RulesPreset`. Define `PresetRuleSet` struct wrapping a `RulesPreset` + optional per-net overrides map. Implement `RoutingRuleSet` for `PresetRuleSet`.

2. Create manufacturer preset files: `jlcpcb.rs` (standard_2layer, standard_4layer, advanced_2layer, advanced_4layer — with JLCPCB capability page URLs), `pcbway.rs` (standard — with PCBWay capability page URL), `oshpark.rs` (2layer, 4layer — with OSHPark design rules URLs). Each function returns `DesignConstraints` with all 30+ fields populated from research values. Include copper weight, board thickness, stackup. Use research-verified values (JLCPCB 2L: 5mil/0.127mm trace, not 6mil).

3. Create `crates/cypcb-rules/src/presets/ipc.rs` with generic IPC tier presets: Class 1 (consumer electronics — relaxed), Class 2 (dedicated service — standard), Class 3 (high reliability — tight). Based on IPC-2221 requirements tables.

4. Create `crates/cypcb-rules/src/clearance_table.rs` implementing IPC-2221 voltage-based clearance lookup. Function `voltage_clearance(voltage_v: f64, coating: CoatingType) -> Nm` returning minimum clearance for given voltage. `CoatingType` enum: Bare, ConformCoat, SeaLevel. Include the standard IPC-2221 Table 6-1 breakpoints (0-15V, 16-30V, 31-50V, 51-100V, etc.).

5. Add tests in each preset file verifying exact values match documented specs. Add tests for `PresetRuleSet` implementing `RoutingRuleSet` correctly — constraint queries, via cost, clearance between nets. Add tests for clearance table covering key voltage breakpoints. Add roundtrip test: `from_name(preset.name()) == preset`.

## Must-Haves

- [ ] 10+ manufacturer presets with source URLs in code comments
- [ ] Every preset populates all `DesignConstraints` fields (no defaults leaking through)
- [ ] `PresetRuleSet` implements `RoutingRuleSet` trait with real constraint lookups
- [ ] IPC-2221 voltage clearance table covers 0V-500V range
- [ ] `RulesPreset::from_name()` supports all aliases (e.g., "jlcpcb", "jlcpcb_2layer", "oshpark")
- [ ] `RulesPreset::all()` returns complete list
- [ ] JLCPCB 2-layer uses corrected 5mil (0.127mm) values, not the old 6mil
- [ ] Each preset includes matching `Stackup` definition

## Verification

- `cargo test -p cypcb-rules` — all preset tests pass
- `cargo test -p cypcb-rules -- presets` — focused preset test output
- `cargo clippy -p cypcb-rules -- -D warnings` — clean
- Verify source URLs are present: `grep -c "Source:" crates/cypcb-rules/src/presets/*.rs` shows URLs in every file

## Observability Impact

- Signals added/changed: None — pure data constructors
- How a future agent inspects this: `RulesPreset::all()` enumerates all presets, each has `.name()` and `.constraints()`
- Failure state exposed: Preset construction is infallible. `from_name()` returns `Option` for unknown names.

## Inputs

- `crates/cypcb-rules/src/constraints.rs` — `DesignConstraints` struct from T01
- `crates/cypcb-rules/src/signal_class.rs` — `SignalClass` from T01
- `crates/cypcb-rules/src/stackup.rs` — `Stackup` types from T01
- `crates/cypcb-rules/src/routing_rules.rs` — `RoutingRuleSet` trait from T01
- S01 research: manufacturer capabilities table, IPC-2221 clearance tables

## Expected Output

- `crates/cypcb-rules/src/presets/mod.rs` — `RulesPreset` enum, `PresetRuleSet` implementing `RoutingRuleSet`
- `crates/cypcb-rules/src/presets/jlcpcb.rs` — 4 JLCPCB preset variants
- `crates/cypcb-rules/src/presets/pcbway.rs` — PCBWay standard preset
- `crates/cypcb-rules/src/presets/oshpark.rs` — 2 OSHPark preset variants
- `crates/cypcb-rules/src/presets/ipc.rs` — 3 IPC class presets
- `crates/cypcb-rules/src/clearance_table.rs` — IPC-2221 voltage clearance lookup
- `crates/cypcb-rules/src/lib.rs` — updated re-exports
