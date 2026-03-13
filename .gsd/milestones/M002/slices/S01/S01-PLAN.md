# S01: PCB Knowledge Base & Design Rules

**Goal:** Comprehensive PCB design rule database exists as both structured documentation (`docs/pcb-knowledge/`) and a typed Rust crate (`crates/cypcb-rules/`) with signal classification, manufacturer presets, stackup definitions, and a `RoutingRuleSet` trait designed for S02's autorouter. Existing DRC extended with new rules and backward-compatible preset expansion.
**Demo:** `cargo test -p cypcb-rules` passes with tests covering all manufacturer presets, signal class constraint lookups, stackup calculations, and IPC clearance tables. `cargo test -p cypcb-drc` passes with new edge clearance, annular ring, and same-net exemption tests. Knowledge base docs exist at `docs/pcb-knowledge/` with IPC standards, manufacturer specs, signal integrity rules, and competitor analysis.

## Must-Haves

- `crates/cypcb-rules/` crate depending only on `cypcb-core`, with `RoutingRuleSet` trait, `DesignConstraints` struct (30+ fields), `SignalClass` enum, `Stackup` definitions, per-net/per-class rule overrides
- Manufacturer presets: JLCPCB standard/advanced 2/4-layer, PCBWay, OSHPark 2/4-layer, generic IPC tiers — all with source URLs in code comments
- IPC-2221 voltage clearance lookup tables encoded as functions
- Signal classification: Digital, HighSpeed, Analog, Power, Differential — with constraint mapping for the autorouter
- `cypcb-drc` extended with edge clearance rule, annular ring rule, same-net clearance exemption, and new `Preset` variants (OSHPark, JLCPCB advanced)
- Backward compatibility: existing `DesignRules` API and `Preset::from_name()` still work unchanged
- `docs/pcb-knowledge/` structured knowledge base with IPC standards, manufacturer capabilities, signal integrity best practices, thermal guidelines, competitor DRC analysis

## Proof Level

- This slice proves: contract
- Real runtime required: no (unit tests and doc inspection only)
- Human/UAT required: no

## Verification

- `cargo test -p cypcb-rules` — all unit tests pass (presets, signal classes, stackups, clearance tables, rule queries)
- `cargo test -p cypcb-drc` — all existing + new tests pass (edge clearance, annular ring, same-net exemption, new presets)
- `cargo build --workspace` — full workspace compiles cleanly with new crate integrated
- `test -d docs/pcb-knowledge && test -f docs/pcb-knowledge/README.md` — knowledge base directory and index exist
- `cargo clippy -p cypcb-rules -- -D warnings` — no clippy warnings in new crate

## Observability / Diagnostics

- Runtime signals: None — this is a pure-data crate with no runtime behavior. DRC violations produce structured `DrcViolation` types with kind, location, and message.
- Inspection surfaces: `Preset::all()` returns all available presets. `SignalClass` enum is exhaustive. `Stackup` types are inspectable via Debug.
- Failure visibility: DRC violations include `ViolationKind`, location `Point`, entity references, and human-readable messages. Compile-time errors if API contracts break.
- Redaction constraints: None — no secrets in PCB design rules.

## Integration Closure

- Upstream surfaces consumed: `cypcb-core` (Nm, Point, units), `cypcb-world` (BoardWorld, Layer — consumed by `cypcb-drc` only, not `cypcb-rules`)
- New wiring introduced in this slice: `cypcb-rules` crate added to workspace, `cypcb-drc` gains dependency on `cypcb-rules` for extended constraint types
- What remains before the milestone is truly usable end-to-end: S02 autorouter must consume `RoutingRuleSet` trait, S03-S08 build on top. This slice is pure foundation.

## Tasks

- [x] **T01: Scaffold cypcb-rules crate with core types, signal classes, and RoutingRuleSet trait** `est:2h`
  - Why: The `RoutingRuleSet` trait and `DesignConstraints` struct are the highest-risk API — S02's autorouter depends on them. Getting the type design right first de-risks everything downstream.
  - Files: `crates/cypcb-rules/Cargo.toml`, `crates/cypcb-rules/src/lib.rs`, `crates/cypcb-rules/src/constraints.rs`, `crates/cypcb-rules/src/signal_class.rs`, `crates/cypcb-rules/src/stackup.rs`, `crates/cypcb-rules/src/routing_rules.rs`, `Cargo.toml`
  - Do: Create crate with `cypcb-core` dependency only. Define `DesignConstraints` with 30+ fields (clearance, trace width, drill, via, annular ring, silk, edge clearance, impedance, differential pair gap, thermal relief, via fanout, copper weight, board thickness, etc.). Define `SignalClass` enum with constraint mapping. Define `Stackup` and `LayerStack` types with dielectric constants. Define `RoutingRuleSet` trait with methods `constraints_for_net()`, `constraints_for_class()`, `via_cost()`, `layer_change_cost()`, `clearance_between()`. Add comprehensive unit tests for all types.
  - Verify: `cargo test -p cypcb-rules` passes, `cargo clippy -p cypcb-rules -- -D warnings` clean
  - Done when: All core types compile, trait is object-safe, 20+ unit tests pass covering type construction and constraint queries

- [x] **T02: Manufacturer presets, IPC clearance tables, and preset implementations** `est:2h`
  - Why: The presets are the primary consumer-facing API — every board design starts by selecting a manufacturer preset. IPC clearance tables are referenced by multiple rules.
  - Files: `crates/cypcb-rules/src/presets/mod.rs`, `crates/cypcb-rules/src/presets/jlcpcb.rs`, `crates/cypcb-rules/src/presets/pcbway.rs`, `crates/cypcb-rules/src/presets/oshpark.rs`, `crates/cypcb-rules/src/presets/ipc.rs`, `crates/cypcb-rules/src/clearance_table.rs`, `crates/cypcb-rules/src/lib.rs`
  - Do: Implement all manufacturer presets with verified specs and source URLs in comments: JLCPCB standard 2L/4L, JLCPCB advanced 2L/4L, PCBWay standard, OSHPark 2L/4L, generic IPC Class 1/2/3. Implement `RoutingRuleSet` for a `PresetRuleSet` struct that wraps presets. Encode IPC-2221 voltage-based clearance tables as lookup functions. Each preset includes full `DesignConstraints` + `Stackup`. Add tests verifying all preset values match documented specs.
  - Verify: `cargo test -p cypcb-rules` passes with preset-specific tests, every preset constructs valid constraints
  - Done when: 10+ manufacturer presets with source-documented values, IPC clearance lookup works, `PresetRuleSet` implements `RoutingRuleSet` trait

- [x] **T03: Extend cypcb-drc with new rules, presets, and backward compatibility** `est:2h`
  - Why: The DRC engine must use the new rules crate while keeping the existing API stable. New rules (edge clearance, annular ring) and the same-net clearance fix are needed for S02 routing validation.
  - Files: `crates/cypcb-drc/Cargo.toml`, `crates/cypcb-drc/src/presets/mod.rs`, `crates/cypcb-drc/src/presets/oshpark.rs`, `crates/cypcb-drc/src/rules/mod.rs`, `crates/cypcb-drc/src/rules/edge_clearance.rs`, `crates/cypcb-drc/src/rules/annular_ring.rs`, `crates/cypcb-drc/src/rules/clearance.rs`, `crates/cypcb-drc/src/violation.rs`
  - Do: Add `cypcb-rules` dependency to `cypcb-drc`. Add `Preset` variants for OSHPark 2L/4L and JLCPCB advanced tiers with `from_name()` aliases. Implement `EdgeClearanceRule` checking copper-to-board-edge distance. Implement `AnnularRingRule` checking (pad_diameter - drill_diameter) / 2 >= min. Fix same-net clearance exemption in `ClearanceRule`. Add `ViolationKind::EdgeClearance` variant. Register new rules in `run_drc()`. Ensure all existing tests still pass unchanged.
  - Verify: `cargo test -p cypcb-drc` all pass (existing + new), `Preset::from_name("oshpark")` returns valid preset, same-net exemption test passes
  - Done when: 2 new DRC rules working with tests, same-net fix verified, new presets accessible via `from_name()`, all existing DRC tests unchanged and passing

- [x] **T04: PCB knowledge base documentation and competitor DRC analysis** `est:1h30m`
  - Why: The knowledge base is the human-readable reference that future slices and contributors consult. Competitor analysis captures patterns (not code) that inform our DRC and autorouter design.
  - Files: `docs/pcb-knowledge/README.md`, `docs/pcb-knowledge/ipc-standards.md`, `docs/pcb-knowledge/manufacturer-capabilities.md`, `docs/pcb-knowledge/signal-integrity.md`, `docs/pcb-knowledge/thermal-management.md`, `docs/pcb-knowledge/trace-geometry.md`, `docs/pcb-knowledge/competitors/README.md`, `docs/pcb-knowledge/competitors/kicad-drc.md`, `docs/pcb-knowledge/competitors/horizon-eda.md`, `docs/pcb-knowledge/competitors/atopile-constraints.md`
  - Do: Create structured markdown knowledge base. IPC standards doc covers IPC-2221 (clearance, trace width, thermal), IPC-2141 (impedance), IPC-2222 (stackup). Manufacturer capabilities doc with comparison table and source URLs. Signal integrity doc with classification taxonomy and routing best practices. Thermal management doc with IPC-2221 current derating and copper pour guidelines. Trace geometry doc with via fanout, differential pair, and length matching patterns. Competitor analysis docs capture DRC architecture patterns from KiCad, Horizon EDA, and atopile — patterns only, no copied code. Each doc includes license/attribution notes.
  - Verify: `find docs/pcb-knowledge -name '*.md' | wc -l` returns >= 10, all docs have content (not stubs), README.md has table of contents linking all docs
  - Done when: Knowledge base covers all 5 topic areas + 3 competitor analyses, each doc has substantive content with sources cited, README index is complete

## Files Likely Touched

- `Cargo.toml` (workspace members)
- `crates/cypcb-rules/Cargo.toml`
- `crates/cypcb-rules/src/lib.rs`
- `crates/cypcb-rules/src/constraints.rs`
- `crates/cypcb-rules/src/signal_class.rs`
- `crates/cypcb-rules/src/stackup.rs`
- `crates/cypcb-rules/src/routing_rules.rs`
- `crates/cypcb-rules/src/presets/mod.rs`
- `crates/cypcb-rules/src/presets/jlcpcb.rs`
- `crates/cypcb-rules/src/presets/pcbway.rs`
- `crates/cypcb-rules/src/presets/oshpark.rs`
- `crates/cypcb-rules/src/presets/ipc.rs`
- `crates/cypcb-rules/src/clearance_table.rs`
- `crates/cypcb-drc/Cargo.toml`
- `crates/cypcb-drc/src/presets/mod.rs`
- `crates/cypcb-drc/src/presets/oshpark.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
- `crates/cypcb-drc/src/rules/edge_clearance.rs`
- `crates/cypcb-drc/src/rules/annular_ring.rs`
- `crates/cypcb-drc/src/rules/clearance.rs`
- `crates/cypcb-drc/src/violation.rs`
- `docs/pcb-knowledge/README.md`
- `docs/pcb-knowledge/ipc-standards.md`
- `docs/pcb-knowledge/manufacturer-capabilities.md`
- `docs/pcb-knowledge/signal-integrity.md`
- `docs/pcb-knowledge/thermal-management.md`
- `docs/pcb-knowledge/trace-geometry.md`
- `docs/pcb-knowledge/competitors/README.md`
- `docs/pcb-knowledge/competitors/kicad-drc.md`
- `docs/pcb-knowledge/competitors/horizon-eda.md`
- `docs/pcb-knowledge/competitors/atopile-constraints.md`
