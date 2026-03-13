---
estimated_steps: 5
estimated_files: 7
---

# T01: Scaffold cypcb-rules crate with core types, signal classes, and RoutingRuleSet trait

**Slice:** S01 — PCB Knowledge Base & Design Rules
**Milestone:** M002

## Description

Create the `cypcb-rules` crate as a leaf dependency (depends only on `cypcb-core`). This crate defines the foundational types that S02's autorouter and the extended DRC engine will consume: `DesignConstraints` (30+ fields covering all PCB fabrication parameters), `SignalClass` enum with constraint mapping, `Stackup`/`LayerStack` types with dielectric properties, and the `RoutingRuleSet` trait designed for A*-based routing cost functions.

This is the highest-risk API design in S01 — if the trait interface is wrong, S02 will force a rework. The trait must support: per-net constraint queries, per-signal-class constraint queries, via cost calculation, layer change cost, and clearance-between-nets queries.

## Steps

1. Create `crates/cypcb-rules/Cargo.toml` with `cypcb-core` workspace dependency only. Add `cypcb-rules` to workspace `Cargo.toml` members (already `crates/*` glob — verify it picks up automatically) and add `cypcb-rules = { path = "crates/cypcb-rules" }` to `[workspace.dependencies]`.

2. Create `crates/cypcb-rules/src/constraints.rs` with `DesignConstraints` struct containing 30+ fields organized by category: basic geometry (min_clearance, min_trace_width, min_drill_size, min_via_drill, min_annular_ring, min_silk_width, min_edge_clearance), advanced geometry (min_via_annular_ring, max_drill_aspect_ratio, min_solder_mask_bridge, min_paste_clearance), signal integrity (default_impedance_ohms, diff_pair_gap, diff_pair_tolerance, max_stub_length, length_match_tolerance), thermal (max_current_per_width, thermal_relief_gap, thermal_relief_spoke_width, min_copper_pour_clearance), and manufacturing (copper_weight_oz, board_thickness, min_hole_to_hole, min_hole_to_edge, blind_vias_allowed, buried_vias_allowed). All dimension fields use `Nm` type. Include `Default` impl using JLCPCB 2-layer values.

3. Create `crates/cypcb-rules/src/signal_class.rs` with `SignalClass` enum (Digital, HighSpeed, Analog, Power, Differential) and `SignalClassConstraints` struct providing per-class overrides (min_trace_width, min_clearance, preferred_layers, special_rules flags). Implement `SignalClass::default_constraints()` returning sensible defaults for each class based on research values.

4. Create `crates/cypcb-rules/src/stackup.rs` with `Stackup` struct (name, layers vec, total_thickness), `LayerStackEntry` (layer type, thickness, material, copper_weight_oz, dielectric_constant), `LayerType` enum (Signal, Plane, Dielectric, SolderMask, Silkscreen). Include factory methods for common stackups: `two_layer_1oz()`, `four_layer_standard()`, `six_layer_standard()`.

5. Create `crates/cypcb-rules/src/routing_rules.rs` with `RoutingRuleSet` trait: `fn constraints_for_net(&self, net_id: u32) -> &DesignConstraints`, `fn constraints_for_class(&self, class: SignalClass) -> SignalClassConstraints`, `fn via_cost(&self, from_layer: u8, to_layer: u8) -> f64`, `fn layer_change_cost(&self, layer: u8) -> f64`, `fn clearance_between(&self, net_a: u32, net_b: u32) -> Nm`. Ensure trait is object-safe. Create `crates/cypcb-rules/src/lib.rs` re-exporting all public types and modules. Add comprehensive unit tests in each module.

## Must-Haves

- [ ] `cypcb-rules` crate compiles with `cypcb-core` as its only dependency (no `cypcb-world`, no `cypcb-drc`)
- [ ] `DesignConstraints` has 30+ fields covering all PCB fabrication parameter categories
- [ ] `SignalClass` enum with 5 variants and per-class default constraint mappings
- [ ] `Stackup` type with factory methods for 2/4/6-layer boards
- [ ] `RoutingRuleSet` trait is object-safe (`dyn RoutingRuleSet` compiles)
- [ ] All fields use `Nm` for dimensions (no raw floats for physical measurements)
- [ ] `Default` for `DesignConstraints` matches JLCPCB 2-layer research values
- [ ] 20+ unit tests pass covering type construction, defaults, and signal class lookups

## Verification

- `cargo test -p cypcb-rules` — all tests pass
- `cargo clippy -p cypcb-rules -- -D warnings` — no warnings
- `cargo build --workspace` — full workspace compiles with new crate
- Grep for `cypcb_world` or `cypcb_drc` in `crates/cypcb-rules/` returns nothing (leaf dependency constraint)

## Observability Impact

- Signals added/changed: None — pure data types with no runtime behavior
- How a future agent inspects this: `cargo doc -p cypcb-rules --open` for API surface, `cargo test -p cypcb-rules -- --nocapture` for test output
- Failure state exposed: Compile-time errors if API contracts break. Tests exercise every constructor and default.

## Inputs

- `crates/cypcb-core/src/units.rs` — `Nm` type for all dimension fields
- `crates/cypcb-world/src/components/physical.rs` — `Layer` enum (referenced conceptually but NOT imported — routing_rules uses u8 layer indices to avoid dependency)
- S01 research: manufacturer specs, signal class definitions, IPC standards

## Expected Output

- `crates/cypcb-rules/Cargo.toml` — new crate manifest
- `crates/cypcb-rules/src/lib.rs` — crate root with re-exports
- `crates/cypcb-rules/src/constraints.rs` — `DesignConstraints` struct (30+ fields)
- `crates/cypcb-rules/src/signal_class.rs` — `SignalClass` enum and constraint mappings
- `crates/cypcb-rules/src/stackup.rs` — `Stackup`, `LayerStackEntry`, `LayerType` types
- `crates/cypcb-rules/src/routing_rules.rs` — `RoutingRuleSet` trait definition
- `Cargo.toml` — workspace dependencies updated (if needed)
