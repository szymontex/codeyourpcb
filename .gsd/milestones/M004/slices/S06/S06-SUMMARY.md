---
id: S06
parent: M004
milestone: M004
provides:
  - generate_variants() multi-strategy routing with ranked scoring
  - VariantConfig/VariantResult types with Serialize
  - auto_route_variants() WASM entry point returning JSON array
  - Variant panel UI with ranked results, hover ghost preview, click-to-select
  - window.__variantPanel debug surface for E2E testability
  - Serialize derives on RouteSegment, ViaPlacement, RoutingResult, RoutingStatus
requires:
  - slice: S02
    provides: score_board() for ranking variants by composite score
  - slice: S03
    provides: RoutingStrategy trait + PathFinder/ImprovedAStar implementations
  - slice: S04
    provides: smooth_routes() applied to each variant's output
  - slice: S05
    provides: AutorouteParams struct for variant configs
affects:
  - S07
key_files:
  - crates/cypcb-autoroute/src/variant.rs
  - crates/cypcb-router/src/types.rs
  - crates/cypcb-render/src/lib.rs
  - crates/cypcb-autoroute/tests/variant_generation.rs
  - viewer/src/variant-panel.ts
  - viewer/src/renderer.ts
  - viewer/src/main.ts
  - viewer/src/wasm.ts
  - viewer/index.html
  - viewer/e2e/variant-panel.spec.ts
key_files_not_in_repo:
  - viewer/src/variant-panel.ts - deleted by a9e8c7a, `refactor(viewer): delete the variant panel, which nothing could reach`
key_decisions:
  - Sequential variant generation on single &mut BoardWorld (bevy_ecs World not Clone)
  - 4 default configs: PathFinder default, PathFinder low-via, ImprovedAStar default, PathFinder high-density
  - std::time::Instant conditional compilation for WASM (#[cfg(not(target_arch = "wasm32"))])
  - console_error_panic_hook for WASM diagnostics
  - Route button falls back to auto_route() if auto_route_variants() WASM panics
  - Hover ghost overlay: cyan traces at 0.4 alpha, active traces dimmed to 0.3 alpha
patterns_established:
  - Variant generation loop: clear_autorouted_traces → route_board → apply_routes → rebuild_spatial_index → score_board → capture → next
  - WASM fallback pattern: try variant generation, catch WASM panic, reload source, fall back to single route
  - Variant panel show/hide/hover/click with debug surface sync
observability_surfaces:
  - window.__variantPanel — { visible, variantCount, activeIndex, hoveredIndex, variants: [{ name, composite }] }
  - tracing::info! per variant (name, composite score, route count, via count)
  - tracing::info! summary (variant count, best name, elapsed ms)
  - tracing::warn! for individual variant routing failures
  - console.log('[Routing] N variants, best: name (score), Ns')
drill_down_paths:
  - .gsd/milestones/M004/slices/S06/tasks/T01-SUMMARY.md
  - .gsd/milestones/M004/slices/S06/tasks/T02-SUMMARY.md
duration: 75m
verification_result: passed
completed_at: 2026-03-14
---

# S06: Variant Generation & Preview UI

**Route button generates 3-4 routing variants with different strategies/params, ranks them by composite score, auto-applies the best, and lets users hover alternatives as ghost overlays on canvas.**

## What Happened

**T01 — Rust variant generation engine and WASM bridge (15m):** Added `serde::Serialize` derives to `RouteSegment`, `ViaPlacement`, `RoutingResult`, and `RoutingStatus` in cypcb-router. Created `variant.rs` module in cypcb-autoroute with `VariantConfig`, `VariantResult`, `default_variant_configs()` (4 configs), and `generate_variants()` — which sequentially clears, routes, applies, rebuilds spatial index, scores, and captures each variant. Best variant auto-applied after ranking. Added `auto_route_variants()` to `PcbEngine` in cypcb-render returning JSON array. 5 unit tests + 5 integration tests on led_blink fixture.

**T02 — Variant panel UI with hover preview and E2E tests (55m):** Added variant panel HTML/CSS in index.html (collapsible panel with variant rows showing name + score + metrics). Created `variant-panel.ts` with show/hide/hover/click handlers and `window.__variantPanel` debug surface. Extended `renderer.ts` with `VariantPreviewData` type and `drawVariantPreview()` — ghost traces at 0.4 alpha cyan, active traces dimmed to 0.3 alpha during preview. Wired Route button in main.ts to call `auto_route_variants()` with fallback to `auto_route()`. Tuning slider re-route clears variant panel. Fixed critical `std::time::Instant` WASM panic with conditional compilation. Added `console_error_panic_hook` for WASM diagnostics. 7 Playwright E2E tests covering panel lifecycle, hover preview, click selection, and debug surface.

## Verification

- `cargo test -p cypcb-autoroute --lib --release` — 123 tests pass (5 new variant unit tests) ✅
- `cargo test --test variant_generation --release` — 5 integration tests pass (all 4 variants succeed on led_blink, sorted correctly) ✅
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — clean ✅
- `cargo check -p cypcb-render` — clean ✅
- `cd viewer && npx playwright test variant-panel.spec.ts` — 7 E2E tests pass ✅
- Failure-path: `auto_route_variants()` returns JSON error on failure; WASM fallback to `auto_route()` works ✅

## Requirements Advanced

- R112 (Routing Variant Generation) — `generate_variants()` produces 4 variants with different strategies/params, scored and ranked
- R113 (Auto-Apply Best Variant with Hover Preview) — best auto-applied, panel shows rankings, hover previews ghost overlay on canvas

## Requirements Validated

- R112 — 4 variants generated sequentially using PathFinder/ImprovedAStar with varied params, ranked by composite score, 5 unit + 5 integration tests + 7 E2E tests prove the full pipeline
- R113 — Route button auto-applies best, panel shows all variants with scores, hovering renders cyan ghost overlay without mutating board state, 7 E2E tests verify panel lifecycle and hover state

## New Requirements Surfaced

- none

## Requirements Invalidated or Re-scoped

- none

## Deviations

- Fixed `std::time::Instant` WASM panic — `Instant::now()` panics in WASM with "time not implemented on this platform". Conditional compilation added (`#[cfg(not(target_arch = "wasm32"))]`). Not in original task plan.
- Added WASM fallback: if `auto_route_variants()` crashes (WASM panic corrupts engine state), source is reloaded to reset, then `auto_route()` is used. Variant panel hidden in fallback path.
- Added `console_error_panic_hook` crate to cypcb-render for better WASM error diagnostics.

## Known Limitations

- Clicking a non-active variant in the panel marks it as "active" in UI but doesn't re-apply that variant's routes to the board. Would need per-variant re-routing API. Currently click is display-only.
- Variant generation takes ~100s on led_blink in native release mode (4 variants × routing). In WASM, ~1s for simple 2-component board — complex boards may be slow.
- Integration tests take ~100s due to 5 independent tests each generating 4 variants.

## Follow-ups

- S07 will validate variant generation across all benchmark fixtures and measure per-variant timing
- Per-variant apply-on-click (re-route with specific config) deferred — current hover preview is sufficient for R113

## Files Created/Modified

- `crates/cypcb-router/Cargo.toml` — Added serde dependency
- `crates/cypcb-router/src/types.rs` — Serialize derives on RouteSegment, ViaPlacement, RoutingResult, RoutingStatus
- `crates/cypcb-autoroute/src/variant.rs` — New: VariantConfig, VariantResult, generate_variants(), default_variant_configs() + 5 unit tests
- `crates/cypcb-autoroute/src/lib.rs` — Added `pub mod variant;`
- `crates/cypcb-render/src/lib.rs` — New `auto_route_variants()` method + console_error_panic_hook init
- `crates/cypcb-render/Cargo.toml` — Added console_error_panic_hook dependency
- `crates/cypcb-autoroute/tests/variant_generation.rs` — New: 5 integration tests
- `viewer/src/variant-panel.ts` — New: variant panel module with show/hide/hover/click + debug surface
- `viewer/index.html` — Variant panel HTML structure + CSS
- `viewer/src/renderer.ts` — VariantPreviewData, drawVariantPreview(), trace dimming
- `viewer/src/main.ts` — Route button → auto_route_variants(), variant panel wiring, tuning clears panel
- `viewer/src/wasm.ts` — auto_route_variants() on PcbEngine interface and all implementations
- `viewer/e2e/variant-panel.spec.ts` — New: 7 E2E tests

## Forward Intelligence

### What the next slice should know
- `generate_variants()` returns `Vec<VariantResult>` sorted by composite score (lowest = best). Each result has name, score, serialized routes, and vias.
- `auto_route_variants()` WASM entry point returns JSON string: either `[{name, score, routes, vias}, ...]` on success or `{"ok":false,"error":"..."}` on failure.
- The 4 default configs are: PathFinder (default), PathFinder (low-via), ImprovedAStar (default), PathFinder (high-density). S07 should benchmark all 4 across fixtures.
- Integration tests take ~100s in release — plan CI time accordingly.

### What's fragile
- WASM variant generation timing — `std::time::Instant` removed with conditional compilation, so WASM builds have no timing data. S07 benchmarks should run natively.
- Variant click doesn't re-apply routes — only hover preview works for visual comparison. If S07 needs to switch between applied variants, it'll need a per-variant apply API.

### Authoritative diagnostics
- `window.__variantPanel` in browser console — shows variant count, active index, hovered index, and all variant names with scores
- `RUST_LOG=cypcb_autoroute::variant=info cargo test --test variant_generation --release -- --nocapture` — shows per-variant timing and scores

### What assumptions changed
- Assumed BoardWorld could be cheaply cloned for parallel variant generation — bevy_ecs World does NOT implement Clone; had to use sequential clear→route→capture loop
- Assumed std::time would work in WASM — it panics; needed conditional compilation
