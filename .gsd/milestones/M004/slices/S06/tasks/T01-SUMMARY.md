---
id: T01
parent: S06
milestone: M004
provides:
  - generate_variants() function for multi-variant routing
  - VariantConfig and VariantResult types with Serialize
  - auto_route_variants() WASM entry point
  - Serialize derives on RouteSegment, ViaPlacement, RoutingResult, RoutingStatus
key_files:
  - crates/cypcb-autoroute/src/variant.rs
  - crates/cypcb-router/src/types.rs
  - crates/cypcb-render/src/lib.rs
  - crates/cypcb-autoroute/tests/variant_generation.rs
key_decisions:
  - Sequential variant generation with clear/route/apply/score cycle (BoardWorld not Clone)
  - 4 default configs: PathFinder default, PathFinder low-via, ImprovedAStar default, PathFinder high-density
  - Best variant auto-applied after all variants scored and sorted
patterns_established:
  - Variant generation loop: clear_autorouted_traces → route_board → apply_routes → rebuild_spatial_index → score_board → capture → next
  - VariantResult serialized as JSON array for WASM bridge
observability_surfaces:
  - tracing::info! per variant (name, composite score, route count, via count)
  - tracing::info! summary (variant count, best name, elapsed ms)
  - tracing::warn! for individual variant routing failures
duration: 15m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Rust variant generation engine and WASM bridge

**Built multi-variant routing engine: 4 strategy/param configs scored sequentially, ranked by composite, best auto-applied, with WASM bridge returning JSON array.**

## What Happened

1. Added `serde` dependency to `cypcb-router` and derived `Serialize` on `RouteSegment`, `ViaPlacement`, `RoutingResult`, and `RoutingStatus`.

2. Created `crates/cypcb-autoroute/src/variant.rs` with:
   - `VariantConfig` (name, strategy, params)
   - `VariantResult` (name, score, routes, vias) with Serialize
   - `default_variant_configs()` returning 4 configs
   - `generate_variants()` implementing the sequential clear→route→apply→rebuild→score→capture loop
   - Internal `clear_autorouted_traces()` and `rebuild_spatial_index()` helpers
   - 5 unit tests

3. Added `auto_route_variants()` to `PcbEngine` in cypcb-render following the existing `auto_route()` pattern. Returns JSON array of VariantResult or `{"ok":false,"error":"..."}`.

4. Created integration test `variant_generation.rs` with 5 tests on the led_blink fixture: multiple results, sorted by composite, all have routes, best applied to world, JSON serialization roundtrip.

## Verification

- `cargo test -p cypcb-autoroute --lib --release` — 123 tests pass (5 new variant unit tests) ✅
- `cargo test --test variant_generation --release` — 5 integration tests pass (all 4 variants succeed on led_blink, sorted correctly) ✅
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compiles clean ✅
- `cargo check -p cypcb-render` — auto_route_variants() compiles ✅
- `cargo check -p cypcb-router` — Serialize derives don't break downstream ✅

Slice-level checks status:
- ✅ `cargo test -p cypcb-autoroute --lib --release`
- ✅ `cargo test --test variant_generation --release`
- ✅ `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown`
- ✅ `cargo check -p cypcb-render`
- ⏳ `cd viewer && npx playwright test variant-panel.spec.ts` — not yet created (T02)
- ✅ Failure-path check: auto_route_variants() returns JSON error on failure; generate_variants() logs warn and skips failed variants

## Diagnostics

- Inspect variant generation: `RUST_LOG=cypcb_autoroute::variant=info cargo test --test variant_generation --release -- --nocapture`
- Each variant logs: name, composite score, route count, via count
- Summary log: variant count, best name, elapsed time in ms
- Failed variants logged with tracing::warn! and skipped (partial results returned)
- WASM error path: returns `{"ok":false,"error":"..."}` JSON string

## Deviations

- `parse_kicad_pcb` returns `KicadPcbParseResult` with `.world` and `.library` fields, not bare `BoardWorld`. Integration test adjusted to destructure accordingly.

## Known Issues

- Integration tests take ~100s in release mode due to 5 tests each generating 4 variants on led_blink. Each test parses the fixture independently — could share setup but tests are independent by design.

## Files Created/Modified

- `crates/cypcb-router/Cargo.toml` — Added serde dependency
- `crates/cypcb-router/src/types.rs` — Added Serialize derives on RouteSegment, ViaPlacement, RoutingResult, RoutingStatus
- `crates/cypcb-autoroute/src/variant.rs` — New: VariantConfig, VariantResult, generate_variants(), default_variant_configs() + 5 unit tests
- `crates/cypcb-autoroute/src/lib.rs` — Added `pub mod variant;`
- `crates/cypcb-render/src/lib.rs` — New `auto_route_variants()` method on PcbEngine
- `crates/cypcb-autoroute/tests/variant_generation.rs` — New: 5 integration tests (~170 LOC)
- `.gsd/milestones/M004/slices/S06/S06-PLAN.md` — Added failure-path verification check
