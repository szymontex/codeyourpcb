---
id: T03
parent: S01
milestone: M002
provides:
  - EdgeClearanceRule DRC rule checking copper-to-board-edge distance
  - AnnularRingRule DRC rule validating (pad_size - drill_size) / 2
  - Same-net clearance exemption in ClearanceRule preventing false positives
  - 4 new Preset variants (OshPark2Layer, OshPark4Layer, JlcpcbAdvanced2Layer, JlcpcbAdvanced4Layer)
  - Case-insensitive and hyphen-tolerant Preset::from_name()
  - EdgeClearance ViolationKind variant with constructor
  - DrcViolation::annular_ring() and DrcViolation::edge_clearance() constructors
key_files:
  - crates/cypcb-drc/src/rules/edge_clearance.rs
  - crates/cypcb-drc/src/rules/annular_ring.rs
  - crates/cypcb-drc/src/rules/clearance.rs
  - crates/cypcb-drc/src/presets/oshpark.rs
  - crates/cypcb-drc/src/presets/mod.rs
  - crates/cypcb-drc/src/violation.rs
  - crates/cypcb-drc/src/lib.rs
key_decisions:
  - Edge clearance uses board bounding box (origin at 0,0) rather than outline geometry — sufficient for rectangular boards, matches BoardSize semantics
  - Annular ring uses min(pad_width, pad_height) for worst-case check on non-circular pads
  - Same-net exemption builds HashMap<entity_index, NetId> upfront for O(1) lookups during clearance checking
  - OSHPark/JLCPCB advanced preset values in cypcb-drc match the 7-field DesignRules subset of the 35-field cypcb-rules DesignConstraints
  - from_name() now normalizes input (lowercase + hyphen-to-underscore) for case/separator-insensitive matching
patterns_established:
  - DRC rule pattern — stateless struct implementing DrcRule trait, no-op on empty world or missing board context
  - DrcViolation constructor pattern — named constructors (edge_clearance, annular_ring) with actual-vs-required dimensions in message
observability_surfaces:
  - ViolationKind::EdgeClearance — new variant in DRC output for copper-to-edge violations
  - ViolationKind::AnnularRing — now produced by real rule (was placeholder)
  - DrcViolation messages include actual and required dimensions for both new rules
  - with_pad_info() now works for AnnularRing violations (adds refdes.pad to message)
duration: 25min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T03: Extend cypcb-drc with new rules, presets, and backward compatibility

**Added EdgeClearance and AnnularRing DRC rules, 4 new manufacturer presets, and same-net clearance exemption — 99 lib tests + 23 doc tests pass, all existing tests unchanged.**

## What Happened

Extended the cypcb-drc crate in five areas:

1. **New presets**: Added `OshPark2Layer`, `OshPark4Layer`, `JlcpcbAdvanced2Layer`, `JlcpcbAdvanced4Layer` variants to the `Preset` enum. Created `presets/oshpark.rs` with constructors for all four new presets. Values sourced from `cypcb-rules` crate (the 7-field DRC subset of the 35-field DesignConstraints). `from_name()` now normalizes input for case/separator insensitivity. `Preset::all()` returns 8 presets.

2. **EdgeClearanceRule**: Checks all spatial entries against the four board edges (x=0, y=0, x=width, y=height). Silently passes when no board is defined. Reports `ViolationKind::EdgeClearance` with actual/required distances.

3. **AnnularRingRule**: Iterates components with through-hole pads via FootprintLibrary, computes `(min(pad_w, pad_h) - drill) / 2`, compares against `min_annular_ring`. SMD pads are exempt. Reports `ViolationKind::AnnularRing`.

4. **Same-net clearance exemption**: Fixed the TODO in `ClearanceRule.check()`. Builds a `HashMap<entity_index, NetId>` from the ECS at the start of each check, then skips pairs where both entities share the same NetId. Entities without a NetId component are still checked (backward compatible).

5. **Registration**: Both new rules added to the `run_drc()` checkers vec alongside existing rules.

## Verification

- `cargo test -p cypcb-drc` — 99 lib tests + 23 doc tests pass (0 failures)
- `cargo test -p cypcb-drc -- preset` — 15 preset-specific tests pass (includes new variants, roundtrip, case insensitivity)
- `cargo test -p cypcb-drc -- edge_clearance` — 10 edge clearance tests pass (all edges, boundary, multiple violations, no board)
- `cargo test -p cypcb-drc -- annular_ring` — 7 annular ring tests pass (pass/fail/SMD exempt/unknown footprint/message dims)
- `cargo test -p cypcb-drc -- clearance` — 25 clearance tests pass (includes 3 new same-net exemption tests)
- `cargo build -p cypcb-drc -p cypcb-rules -p cypcb-core -p cypcb-world -p cypcb-parser` — clean build
- `cargo clippy -p cypcb-drc` — no warnings from cypcb-drc (upstream crate warnings only)

### Slice-level verification status (intermediate task):
- ✅ `cargo test -p cypcb-rules` — all tests pass
- ✅ `cargo test -p cypcb-drc` — all existing + new tests pass
- ✅ core workspace compiles cleanly
- ❌ `docs/pcb-knowledge/` — not yet created (T04 scope)
- ⚠️ `cargo clippy -p cypcb-rules -- -D warnings` — blocked by pre-existing cypcb-core derive clippy error

## Diagnostics

- Filter DRC violations by `ViolationKind::EdgeClearance` or `ViolationKind::AnnularRing`
- Each violation includes actual vs. required dimensions in its message
- `Preset::all()` returns all 8 presets for enumeration
- `Preset::from_name()` returns `None` for unknown names — no panics
- `cargo test -p cypcb-drc -- --nocapture` for full test output

## Deviations

- OSHPark minimum drill (0.254mm) is actually smaller than JLCPCB's (0.3mm) — corrected the test that assumed otherwise
- DIP-8 annular ring is 0.4mm (pad=1.6mm, drill=0.8mm), not 0.3mm as initially assumed — adjusted test thresholds accordingly
- `with_pad_info()` extended to handle AnnularRing violations (not just DrillSize) since both rules iterate footprint pads

## Known Issues

- EdgeClearanceRule assumes rectangular board at origin (0,0) — complex board outlines would need polygon-based distance calculation (future scope)
- Full workspace `cargo build --workspace` fails due to pre-existing GTK/GDK system library dependency in renderer crate (unrelated)

## Files Created/Modified

- `crates/cypcb-drc/Cargo.toml` — added `cypcb-rules` dependency
- `crates/cypcb-drc/src/presets/oshpark.rs` — **new** OSHPark + JLCPCB advanced preset constructors with tests
- `crates/cypcb-drc/src/presets/mod.rs` — 4 new Preset variants, expanded from_name/name/rules/all
- `crates/cypcb-drc/src/rules/edge_clearance.rs` — **new** EdgeClearanceRule implementation with 10 tests
- `crates/cypcb-drc/src/rules/annular_ring.rs` — **new** AnnularRingRule implementation with 7 tests
- `crates/cypcb-drc/src/rules/clearance.rs` — same-net exemption via HashMap<entity_index, NetId>, 3 new tests
- `crates/cypcb-drc/src/rules/mod.rs` — new rule module declarations and re-exports
- `crates/cypcb-drc/src/violation.rs` — EdgeClearance variant, edge_clearance() + annular_ring() constructors, with_pad_info() extended
- `crates/cypcb-drc/src/lib.rs` — EdgeClearanceRule + AnnularRingRule registered in run_drc()
