---
estimated_steps: 5
estimated_files: 8
---

# T03: Extend cypcb-drc with new rules, presets, and backward compatibility

**Slice:** S01 — PCB Knowledge Base & Design Rules
**Milestone:** M002

## Description

Wire the new `cypcb-rules` crate into `cypcb-drc`. Add new DRC rules (edge clearance, annular ring), fix the same-net clearance exemption, and expand the `Preset` enum with OSHPark and JLCPCB advanced variants. All changes must preserve backward compatibility — existing `DesignRules`, `Preset::from_name()`, and all existing tests must continue to work unchanged.

## Steps

1. Add `cypcb-rules` as a dependency in `crates/cypcb-drc/Cargo.toml`. Update `crates/cypcb-drc/src/presets/mod.rs`: add `Preset` variants `OshPark2Layer`, `OshPark4Layer`, `JlcpcbAdvanced2Layer`, `JlcpcbAdvanced4Layer`. Add `from_name()` aliases ("oshpark", "oshpark_2layer", "oshpark_4layer", "jlcpcb_advanced", "jlcpcb_advanced_2layer", "jlcpcb_advanced_4layer"). Add constructor methods on `DesignRules` for new presets. Create `crates/cypcb-drc/src/presets/oshpark.rs` with OSHPark values. Update `Preset::all()` and `Preset::name()`.

2. Create `crates/cypcb-drc/src/rules/edge_clearance.rs` implementing `DrcRule`. Check distance from every copper feature (component pads, via pads) to the board outline. If the board has an outline defined, check that all copper is at least `min_edge_clearance` from the edge. Use bounding box of the board outline from `BoardWorld`. Add `ViolationKind::EdgeClearance` to `violation.rs`.

3. Create `crates/cypcb-drc/src/rules/annular_ring.rs` implementing `DrcRule`. For each pad with a drill hole, verify that `(pad_diameter - drill_diameter) / 2 >= min_annular_ring`. Query pads from BoardWorld ECS, compute annular ring from pad size and drill size. Add `ViolationKind::AnnularRing` variant if not already present (it exists as a placeholder — keep it, just use it).

4. Fix same-net clearance exemption in `crates/cypcb-drc/src/rules/clearance.rs`. The existing clearance rule has a TODO for same-net exemption — pads on the same net that are close together should NOT generate clearance violations. Check the `NetId` component on both entities; if they share the same net, skip the clearance check. This must work for pad-to-pad, pad-to-trace, and trace-to-trace pairs.

5. Register new rules in `run_drc()` in `crates/cypcb-drc/src/lib.rs`. Add `EdgeClearanceRule` and `AnnularRingRule` to the checkers vec. Add comprehensive tests: edge clearance violation and pass cases, annular ring violation and pass cases, same-net exemption test, new preset roundtrip tests. Verify all existing tests still pass.

## Must-Haves

- [ ] `EdgeClearanceRule` detects copper too close to board edge
- [ ] `AnnularRingRule` validates (pad_size - drill_size) / 2 relationship
- [ ] Same-net clearance exemption prevents false positives on connected pads
- [ ] `Preset::from_name("oshpark")` returns `OshPark2Layer`
- [ ] `Preset::from_name("jlcpcb_advanced")` returns `JlcpcbAdvanced2Layer`
- [ ] All existing DRC tests pass unchanged
- [ ] New rules registered in `run_drc()` checker list
- [ ] `Preset::all()` includes all new variants

## Verification

- `cargo test -p cypcb-drc` — all tests pass (existing + new)
- `cargo test -p cypcb-drc -- preset` — preset-specific tests
- `cargo test -p cypcb-drc -- edge_clearance` — new rule tests
- `cargo test -p cypcb-drc -- annular_ring` — new rule tests
- `cargo test -p cypcb-drc -- clearance` — same-net exemption test
- `cargo build --workspace` — full workspace still compiles

## Observability Impact

- Signals added/changed: New `ViolationKind::EdgeClearance` variant in DRC output. Existing `ViolationKind::AnnularRing` now produced by real rule (was placeholder).
- How a future agent inspects this: DRC violations include kind, location, entity references, and descriptive message. Filter violations by `ViolationKind` to isolate specific rule failures.
- Failure state exposed: Each new rule produces structured `DrcViolation` with exact location and dimensions (actual vs. required values in message).

## Inputs

- `crates/cypcb-rules/` — constraint types and preset values from T01/T02
- `crates/cypcb-drc/src/rules/clearance.rs` — existing clearance rule with same-net TODO
- `crates/cypcb-drc/src/presets/mod.rs` — existing `Preset` enum and `DesignRules` struct
- `crates/cypcb-world/` — `BoardWorld` ECS queries for pads, traces, board outline

## Expected Output

- `crates/cypcb-drc/Cargo.toml` — `cypcb-rules` dependency added
- `crates/cypcb-drc/src/presets/mod.rs` — expanded `Preset` enum with new variants
- `crates/cypcb-drc/src/presets/oshpark.rs` — OSHPark preset constructors
- `crates/cypcb-drc/src/rules/edge_clearance.rs` — new DRC rule
- `crates/cypcb-drc/src/rules/annular_ring.rs` — new DRC rule
- `crates/cypcb-drc/src/rules/clearance.rs` — same-net exemption fix
- `crates/cypcb-drc/src/rules/mod.rs` — new rule exports
- `crates/cypcb-drc/src/violation.rs` — `EdgeClearance` variant added
- `crates/cypcb-drc/src/lib.rs` — new rules registered in `run_drc()`
