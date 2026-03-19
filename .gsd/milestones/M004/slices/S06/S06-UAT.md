# S06: Variant Generation & Preview UI — UAT

**Milestone:** M004
**Written:** 2026-03-14

## UAT Type

- UAT mode: mixed (artifact-driven for Rust engine + live-runtime for browser UI)
- Why this mode is sufficient: Rust engine verified by integration tests on real benchmark fixtures; browser UI verified by E2E tests on debug surfaces. Human-visual verification optional — ghost overlay appearance is a style preference, not correctness.

## Preconditions

- Rust toolchain with `wasm32-unknown-unknown` target installed
- WASM built: `cd crates/cypcb-render && wasm-pack build --target web --release`
- Viewer dev server running: `cd viewer && npx vite`
- A `.cypcb` file loaded in the viewer (any board with components and nets)
- Browser DevTools console available for inspecting debug surfaces

## Smoke Test

1. Open the viewer in browser with any board loaded
2. Click the "Route" button
3. **Expected:** A "Routing Variants" panel appears in the bottom-right showing 3-4 variants with names and composite scores, ranked best-first. The board displays routed traces.

## Test Cases

### 1. Variant generation produces multiple ranked results

1. Load `tests/fixtures/blink.cypcb` or any board with nets
2. Click "Route" button
3. **Expected:** Variant panel appears with 3-4 entries. Each entry has a strategy name (e.g. "PathFinder (default)", "ImprovedAStar (default)") and a composite score. Entries are sorted by score (lowest/best first). First entry is highlighted as active.

### 2. Best variant auto-applied to canvas

1. Load a board and click "Route"
2. Observe the canvas after routing completes
3. **Expected:** Traces are visible on the canvas. The active (first) variant's routing is what's displayed. The variant panel's first entry shows as selected/highlighted.

### 3. Hover preview shows ghost overlay

1. After routing (variant panel visible), hover over a non-active variant row
2. **Expected:** Ghost traces appear on canvas in cyan color at reduced opacity (~0.4 alpha). The existing active traces dim to ~0.3 alpha. The ghost overlay represents the hovered variant's different routing.
3. Move mouse away from the variant row
4. **Expected:** Ghost overlay disappears, active traces return to full opacity.

### 4. Click selects a different active variant

1. After routing, click on a non-active variant in the panel
2. **Expected:** The clicked variant becomes highlighted as active. The previously active variant loses its highlight. `window.__variantPanel.activeIndex` updates to the clicked variant's index.

### 5. Variant panel clears on new Route click

1. After routing (variant panel visible), click "Route" again
2. **Expected:** Variant panel clears briefly (or shows loading state), then repopulates with fresh results. New routing may produce different scores.

### 6. Tuning slider re-route clears variant panel

1. After routing (variant panel visible), open the tuning panel (⚡ button)
2. Adjust any slider (via cost, density, etc.)
3. **Expected:** The variant panel hides/clears. The board re-routes with the single tuning configuration (not variant generation). Only variant generation via "Route" button populates the panel.

### 7. Debug surface reflects state accurately

1. After routing, open browser DevTools console
2. Type `window.__variantPanel`
3. **Expected:** Object with: `visible: true`, `variantCount: 3` or `4`, `activeIndex: 0`, `variants: [{ name: "...", composite: N }, ...]`
4. Hover a variant, check `window.__variantPanel.hoveredIndex`
5. **Expected:** `hoveredIndex` matches the index of the hovered variant (0-based)

### 8. Rust integration: variants sorted by composite score

1. Run `cargo test --test variant_generation --release -- variants_sorted_by_composite_score`
2. **Expected:** Test passes. All variants are returned in ascending composite score order (best first).

### 9. Rust integration: best variant applied to world

1. Run `cargo test --test variant_generation --release -- best_variant_applied_to_world`
2. **Expected:** Test passes. After `generate_variants()`, the BoardWorld contains traces from the best-scoring variant.

## Edge Cases

### WASM fallback on variant generation failure

1. If `auto_route_variants()` encounters a WASM panic (simulate by corrupting engine state)
2. **Expected:** Fallback to `auto_route()` — board still routes successfully. Variant panel remains hidden. Console shows error message from `console_error_panic_hook`.

### Board with no nets

1. Load a board with components but no net connections
2. Click "Route"
3. **Expected:** Variant panel may show variants with score 0 or very low, or may not appear if routing returns no results. No crash or unhandled error.

### Rapid Route button clicks

1. Click "Route" rapidly 3-4 times in succession
2. **Expected:** No crashes, no duplicate panels. Last routing result is displayed. Panel reflects final state.

### Hover then quickly click Route

1. While hovering a variant (ghost overlay visible), click "Route"
2. **Expected:** Ghost overlay clears, variant panel resets, new routing starts. No stale ghost traces remain on canvas.

## Failure Signals

- Variant panel does not appear after clicking Route — check browser console for WASM errors
- Only 1 variant shown — check if `generate_variants()` is being called (vs `auto_route()`)
- Ghost overlay not visible on hover — check `renderState.variantPreview` is being set in main.ts
- `window.__variantPanel` is undefined — variant-panel.ts not initialized
- Scores show as NaN or 0 — check `score_board()` integration in variant generation loop
- WASM panic without message — check `console_error_panic_hook` is initialized

## Requirements Proved By This UAT

- R112 (Routing Variant Generation) — Tests 1, 8, 9 prove multiple variants generated with different strategies, scored, and ranked
- R113 (Auto-Apply Best Variant with Hover Preview) — Tests 2, 3, 4, 7 prove best auto-applied, hover shows preview overlay, click selects, debug surface testable

## Not Proven By This UAT

- Variant generation performance on complex boards (>10 components) — deferred to S07 benchmarks
- Visual quality of ghost overlay rendering (cyan color, opacity levels) — style preference, verified by E2E state checks not pixel comparison
- Per-variant apply-on-click (re-routing with clicked variant's config) — known limitation, click is display-only

## Notes for Tester

- Variant generation in WASM is fast (~1s for simple boards) but native integration tests take ~100s because they run 4 variants on led_blink. This is normal.
- The 4 default variant configs are: PathFinder (default), PathFinder (low-via), ImprovedAStar (default), PathFinder (high-density). Not all may produce different routes on simple boards.
- Ghost overlay uses Canvas 2D `globalAlpha` — in headless browser tests this is verified via debug surface state, not visual pixel checks.
- The `console_error_panic_hook` crate makes WASM panics readable in the console — if you see "unreachable" without a message, the hook may not be initialized.
