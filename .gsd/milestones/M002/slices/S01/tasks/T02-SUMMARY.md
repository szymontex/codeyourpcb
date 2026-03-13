---
id: T02
parent: S01
milestone: M002
provides:
  - 10 manufacturer/IPC presets (JLCPCB ×4, PCBWay, OSHPark ×2, IPC Class 1/2/3)
  - PresetRuleSet implementing RoutingRuleSet trait with per-net overrides
  - IPC-2221 voltage-based clearance lookup covering 0V-500V+ range
  - RulesPreset enum with from_name() alias lookup and all()/name() API
key_files:
  - crates/cypcb-rules/src/presets/mod.rs
  - crates/cypcb-rules/src/presets/jlcpcb.rs
  - crates/cypcb-rules/src/presets/pcbway.rs
  - crates/cypcb-rules/src/presets/oshpark.rs
  - crates/cypcb-rules/src/presets/ipc.rs
  - crates/cypcb-rules/src/clearance_table.rs
  - crates/cypcb-rules/src/lib.rs
key_decisions:
  - PresetRuleSet uses HashMap<u32, DesignConstraints> for per-net overrides rather than a closure or trait-based approach — simple, serializable, and sufficient for the autorouter's needs
  - Via cost in PresetRuleSet scales linearly with layer span and adds 2x premium for blind/buried vias when allowed — keeps cost model simple for A* router integration
  - clearance_between() uses the stricter clearance of the two nets' constraints — conservative approach for safety-critical clearance decisions
  - IPC clearance table uses f64 voltage input (not Nm) since voltage isn't a dimension — consistent with the crate's philosophy of using appropriate types
patterns_established:
  - Manufacturer preset pattern — each preset file exports constraint + stackup function pairs, enum variant dispatches to them
  - from_name() normalization — lowercase + hyphen-to-underscore for case/separator-insensitive alias matching
  - Voltage clearance breakpoint lookup — table-driven with linear extrapolation above 500V
observability_surfaces:
  - RulesPreset::all() enumerates all 10 presets, each has .name() and .constraints()/.stackup()
  - PresetRuleSet::preset() exposes the underlying preset for inspection
  - clearance_table() returns the raw IPC-2221 breakpoint table for display
duration: 1 step
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: Manufacturer presets, IPC clearance tables, and preset implementations

**Built 10 manufacturer/IPC presets with source URLs, PresetRuleSet implementing RoutingRuleSet, and IPC-2221 voltage clearance table — 90 unit tests + 8 doc tests pass, clippy clean.**

## What Happened

Created `presets/` module with 4 files and a clearance table module:

- **presets/mod.rs**: `RulesPreset` enum with 10 variants, `from_name()` supporting 25+ aliases (case-insensitive, hyphen/underscore interchangeable), `constraints()` and `stackup()` dispatchers, `all()` and `name()` for enumeration. `PresetRuleSet` struct wrapping a preset with `HashMap<u32, DesignConstraints>` for per-net overrides, implementing `RoutingRuleSet` with real via cost calculation (linear span scaling + blind/buried premium), layer change cost (outer-preferred), and clearance_between (stricter-of-two-nets).

- **presets/jlcpcb.rs**: 4 variants — standard 2L (5mil/0.127mm, not 6mil), standard 4L (4mil/0.1mm), advanced 2L (3.5mil/0.09mm), advanced 4L (3.5mil + blind vias). Each with JLC7628 prepreg stackup definitions. All 35 DesignConstraints fields populated from JLCPCB capabilities page.

- **presets/pcbway.rs**: Standard process — 6mil trace, 0.2mm drill, blind/buried vias allowed, up to 14 layers.

- **presets/oshpark.rs**: 2-layer (6mil, 10mil drill, FR-408 dielectric) and 4-layer (5mil, 10mil drill, FR-408). Purple solder mask ENIG boards with conservative edge clearance (15mil).

- **presets/ipc.rs**: Class 1 (8mil relaxed), Class 2 (6mil standard), Class 3 (4mil tight). Ordered strictness verified in tests. Generic 2-layer FR-4 stackups.

- **clearance_table.rs**: `voltage_clearance(voltage_v, coating)` implementing IPC-2221B Table 6-1 with 9 breakpoints (0-500V) and linear extrapolation above 500V. `CoatingType` enum: Bare, ConformCoat, SeaLevel. `clearance_table()` returns raw breakpoint data.

## Verification

- `cargo test -p cypcb-rules` — 90 unit tests + 8 doc tests pass
- `cargo test -p cypcb-rules -- presets` — 41 preset tests + 2 doc tests pass
- `cargo clippy -p cypcb-rules -- -D warnings` — clean (pre-existing cypcb-core derivable_impls warning unrelated)
- `grep -c "Source:" crates/cypcb-rules/src/presets/*.rs` — source URLs in every preset file (jlcpcb: 6, oshpark: 5, ipc: 4, pcbway: 2)
- Every preset populates all 35 DesignConstraints fields — verified by `test_every_preset_constraints_populated`
- `from_name(preset.name()) == preset` roundtrip — verified by `test_roundtrip_from_name`
- JLCPCB 2-layer uses 5mil (0.127mm) — verified by `test_standard_2layer_5mil_trace`
- PresetRuleSet is object-safe (dyn dispatch) — verified by `test_preset_ruleset_object_safe`
- IPC clearance table covers 0V-500V+ with extrapolation — verified by `test_extrapolation_above_500v`

### Slice-level checks (partial — T02 is not final task):
- ✅ `cargo test -p cypcb-rules` — 98 tests pass (90 unit + 8 doc)
- ✅ `cargo test -p cypcb-drc` — 17 tests pass (not modified in T02)
- ✅ `cargo build -p cypcb-rules -p cypcb-drc -p cypcb-core` — compiles cleanly
- ⬜ `docs/pcb-knowledge/` — not in T02 scope
- ✅ `cargo clippy -p cypcb-rules -- -D warnings` — clean

## Diagnostics

- `RulesPreset::all()` returns all 10 presets for enumeration
- `RulesPreset::from_name()` returns `Option` for unknown names — no panics
- `clearance_table()` exposes raw IPC-2221 breakpoint data for display/debugging
- Pure data constructors — infallible, no runtime failures possible

## Deviations

None.

## Known Issues

- Pre-existing clippy warning in `cypcb-core/src/units.rs` (derivable_impls) — not introduced by this task.
- `cargo build --workspace` fails due to pre-existing Tauri/GTK dependency issue, not related to cypcb-rules changes.

## Files Created/Modified

- `crates/cypcb-rules/src/presets/mod.rs` — RulesPreset enum (10 variants), PresetRuleSet implementing RoutingRuleSet
- `crates/cypcb-rules/src/presets/jlcpcb.rs` — 4 JLCPCB preset variants with spec URLs
- `crates/cypcb-rules/src/presets/pcbway.rs` — PCBWay standard preset with spec URL
- `crates/cypcb-rules/src/presets/oshpark.rs` — 2 OSHPark preset variants with spec URLs
- `crates/cypcb-rules/src/presets/ipc.rs` — 3 IPC class presets (Class 1/2/3)
- `crates/cypcb-rules/src/clearance_table.rs` — IPC-2221 voltage clearance lookup + CoatingType enum
- `crates/cypcb-rules/src/lib.rs` — updated re-exports for presets and clearance_table modules
