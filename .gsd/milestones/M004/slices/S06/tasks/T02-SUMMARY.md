---
id: T02
parent: S06
milestone: M004
provides:
  - Variant panel UI with ranked routing variants
  - Hover preview overlay (ghost traces in cyan at 0.4 alpha)
  - auto_route_variants() wired to Route button in viewer
  - window.__variantPanel debug surface for E2E testability
  - 7 E2E tests for variant panel flow
key_files:
  - viewer/src/variant-panel.ts
  - viewer/src/renderer.ts
  - viewer/src/main.ts
  - viewer/src/wasm.ts
  - viewer/index.html
  - viewer/e2e/variant-panel.spec.ts
  - crates/cypcb-autoroute/src/variant.rs
  - crates/cypcb-render/src/lib.rs
  - crates/cypcb-render/Cargo.toml
key_files_not_in_repo:
  - viewer/src/variant-panel.ts - deleted by a9e8c7a, `refactor(viewer): delete the variant panel, which nothing could reach`
key_decisions:
  - Fixed std::time::Instant WASM panic by conditionally compiling time measurement (cfg(not(target_arch = "wasm32")))
  - Added console_error_panic_hook for better WASM error diagnostics
  - Route button triggers auto_route_variants() with fallback to auto_route() if variant generation fails
  - Variant preview renders ghost traces at 0.4 alpha in cyan, active traces dimmed to 0.3 alpha during preview
patterns_established:
  - Variant panel pattern: showVariants()/hideVariants() with DOM rows + debug surface sync
  - WASM fallback pattern: try variant generation, catch WASM panic, fall back to single auto_route()
  - Tuning re-route clears variant panel via hideVariants() before routing
observability_surfaces:
  - window.__variantPanel — { visible, variantCount, activeIndex, hoveredIndex, variants: [{ name, composite }] }
  - console.log('[Routing] N variants, best: name (score), Ns') on variant generation
  - console.log('[Variants] Applied variant: name') on click
duration: 55m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: Variant panel UI with hover preview overlay and E2E tests

**Built variant panel showing ranked routing alternatives with hover ghost preview, click-to-select, and 7 passing E2E tests — including fixing std::time::Instant WASM panic in variant generation.**

## What Happened

1. **Variant panel HTML/CSS** (`index.html`): Added collapsible panel in bottom-right of canvas container with header "Routing Variants" and list container. Each variant row shows name, composite score, and metrics (via count, route count). Active variant highlighted with accent color.

2. **variant-panel.ts module**: Created with `initVariantPanel()`, `showVariants()`, `hideVariants()`, `formatScore()`. Panel rows have mouseenter/mouseleave handlers for hover preview and click handler for selecting active variant. `window.__variantPanel` debug surface updated on every state change.

3. **Renderer variant preview** (`renderer.ts`): Added `VariantPreviewData` type and `variantPreview` field to `RenderState`. When preview is active, existing traces are dimmed to 0.3 alpha via `ctx.globalAlpha`, and ghost traces are drawn in cyan (`rgba(0, 200, 255, 0.4)`) with via markers at 0.5 alpha. `drawVariantPreview()` function handles route segments and vias.

4. **WASM bridge** (`wasm.ts`): Added `auto_route_variants()` to `PcbEngine` interface, `WasmPcbEngine` raw interface, `WasmPcbEngineAdapter`, and `MockPcbEngine`.

5. **Main.ts wiring**: Route button now calls `auto_route_variants()` instead of `auto_route()`. Results parsed as VariantData array, stored in `storedVariants`, and shown via `showVariants()`. Hover callback sets `variantPreview` on `RenderState`. Tuning slider re-route clears variant panel. Added fallback: if `auto_route_variants()` throws (WASM crash), falls back to `auto_route()`.

6. **Critical bug fix**: `generate_variants()` used `std::time::Instant::now()` which panics in WASM (`time not implemented on this platform`). Fixed with `#[cfg(not(target_arch = "wasm32"))]` conditional compilation. Also added `console_error_panic_hook` crate for better WASM error diagnostics.

7. **E2E tests**: 7 tests covering: panel initially hidden, route generates variants and shows panel, hover triggers preview state, click makes variant active, panel clears on new Route click, tuning re-route clears panel, debug surface reflects state.

## Verification

- `cargo test -p cypcb-autoroute --lib --release` — 123 tests pass ✅
- `cargo test --test variant_generation --release` — 5 integration tests pass ✅
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — clean ✅
- `cargo check -p cypcb-render` — clean ✅
- `cd viewer && npx playwright test variant-panel.spec.ts` — 7 E2E tests pass ✅
- `cd viewer && npx tsc --noEmit` — clean ✅
- Browser verification: Route button generates 4 variants, panel appears with ranked results, hovering shows ghost preview, debug surface reflects state ✅
- Failure-path check: auto_route_variants() falls back to auto_route() on WASM error ✅

All slice-level verification checks pass:
- ✅ `cargo test -p cypcb-autoroute --lib --release`
- ✅ `cargo test --test variant_generation --release`
- ✅ `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown`
- ✅ `cargo check -p cypcb-render`
- ✅ `cd viewer && npx playwright test variant-panel.spec.ts`
- ✅ Failure-path check

## Diagnostics

- Inspect variant panel state: `window.__variantPanel` in browser console
- Variant panel debug surface: `{ visible, variantCount, activeIndex, hoveredIndex, variants }`
- Console logs: `[Routing] N variants, best: name (score)` on generation
- WASM panic diagnostics: `console_error_panic_hook` provides full panic message + stack trace

## Deviations

- Fixed `std::time::Instant` WASM panic — this was not in the task plan but was blocking variant generation in the browser. Required modifying `crates/cypcb-autoroute/src/variant.rs` and adding `console_error_panic_hook` dependency.
- Added WASM fallback path in `triggerRouting()` — if `auto_route_variants()` throws, falls back to `auto_route()` and skips variant panel. This wasn't planned but makes the feature resilient.

## Known Issues

- Clicking a non-active variant in the panel marks it as "active" in the UI but doesn't actually re-apply that variant's routes to the board. A per-variant apply API would be needed (re-run routing with specific config). Currently variant click is display-only.
- Variant generation in WASM takes ~1s for a simple 2-component board (compared to 85s in native release for led_blink fixture with 7 nets). Performance on complex boards in WASM may be slow.

## Files Created/Modified

- `viewer/src/variant-panel.ts` — New: variant panel module with show/hide/hover/click handlers + debug surface
- `viewer/index.html` — Added variant panel HTML structure and CSS styles
- `viewer/src/renderer.ts` — Added VariantPreviewData type, variantPreview field to RenderState, drawVariantPreview() function, trace dimming when preview active
- `viewer/src/main.ts` — Wired Route button to auto_route_variants(), variant panel init, hover/click callbacks, tuning re-route clears panel
- `viewer/src/wasm.ts` — Added auto_route_variants() to PcbEngine interface and all implementations
- `viewer/e2e/variant-panel.spec.ts` — New: 7 E2E tests for variant panel flow
- `crates/cypcb-autoroute/src/variant.rs` — Fixed std::time::Instant WASM panic with cfg conditional
- `crates/cypcb-render/src/lib.rs` — Added console_error_panic_hook::set_once() in PcbEngine::new()
- `crates/cypcb-render/Cargo.toml` — Added console_error_panic_hook dependency
