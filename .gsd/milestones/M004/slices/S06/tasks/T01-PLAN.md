---
estimated_steps: 6
estimated_files: 6
---

# T01: Rust variant generation engine and WASM bridge

**Slice:** S06 — Variant Generation & Preview UI
**Milestone:** M004

## Description

Build the Rust-side variant generation system: add serde Serialize to router types, implement `generate_variants()` that runs multiple strategy/param configurations sequentially on a single `&mut BoardWorld`, and expose `auto_route_variants()` through the WASM bridge. This provides the engine foundation consumed by T02's UI.

The critical constraint is that `BoardWorld` wraps bevy_ecs `World` which does NOT implement Clone. Variants must be generated sequentially: route → apply → rebuild spatial index → score → serialize route/via data → clear → next variant. The best variant is auto-applied to the world at the end.

## Steps

1. **Add serde to cypcb-router and derive Serialize on route types.** Add `serde = { workspace = true }` to `crates/cypcb-router/Cargo.toml`. Add `#[derive(Serialize)]` to `RouteSegment`, `ViaPlacement`, `RoutingResult`, and `RoutingStatus` in `types.rs`. Import serde::Serialize. Verify `cargo check -p cypcb-router` compiles — all downstream crates must still compile.

2. **Create variant.rs module in cypcb-autoroute.** Define:
   - `VariantConfig { name: String, strategy: StrategyKind, params: AutorouteParams }` 
   - `VariantResult { name: String, score: RoutingScore, routes: Vec<RouteSegment>, vias: Vec<ViaPlacement> }` with Serialize derive
   - `fn default_variant_configs() -> Vec<VariantConfig>` — returns 4 configs: PathFinder default, PathFinder low-via (via_cost=5.0), ImprovedAStar default, PathFinder high-density (density=1.5)
   - `fn generate_variants(world, library, rules, configs) -> Vec<VariantResult>` implementing the sequential loop:
     ```
     for each config:
       clear_autorouted_traces(world)
       route_board(world, library, rules, config_to_autoroute_config(config))
       apply_routes(world, &result)
       rebuild_spatial_index(world)
       score = score_board(world, design_rules, weights)
       capture routes/vias from RoutingResult (before clear)
       collect VariantResult { name, score, routes, vias }
     sort by composite score (ascending = best first)
     apply best variant's routes to world
     ```
   - Use `tracing::info!` to log each variant's name and composite score.
   - Register `pub mod variant;` in lib.rs.

3. **Handle the clear/apply/score lifecycle carefully.** The sequence for each variant must be:
   - `clear_autorouted_traces()` — remove previous variant's entities
   - `route_board()` returns `RoutingResult` — capture the routes/vias data from this
   - `apply_routes()` — write to ECS so score_board() can query entities
   - `rebuild_spatial_index_with_traces()` — needed for crossing detection in scoring
   - `score_board()` — reads from ECS
   - After scoring, store the RoutingResult's routes/vias + score in VariantResult
   - Then clear before next iteration
   
   After all variants are generated, apply the best variant's routes to the world using `apply_routes()` and rebuild.

4. **Add `auto_route_variants()` to PcbEngine in cypcb-render.** Follow the `auto_route()` pattern:
   - Import `generate_variants` and `default_variant_configs` from cypcb-autoroute
   - Call `self.clear_autorouted_traces()` first
   - Build PresetRuleSet + DesignRules
   - Call `generate_variants(&mut self.world, &self.footprint_lib, &rules, &design_rules, &configs)`
   - The function auto-applies the best variant, so just rebuild spatial index + run DRC
   - Serialize the full `Vec<VariantResult>` to JSON and return it
   - Return format: `[{ "name": "...", "score": { ... }, "routes": [...], "vias": [...] }]`
   - On error, return `{"ok":false,"error":"..."}`

5. **Write unit tests in variant.rs.** Test:
   - `default_variant_configs()` returns 4 configs with expected names
   - `VariantConfig` serialization roundtrip
   - `VariantResult` serialization to JSON (verify score, routes, vias fields present)

6. **Write integration test `variant_generation.rs`.** Parse led_blink fixture, call `generate_variants()` with default configs. Assert:
   - Returns 3+ variants (some may fail, but most should succeed)
   - Variants sorted by composite score (ascending)
   - Best variant has lowest composite
   - All variants have non-empty routes
   - After generation, world has routes applied (best variant)

## Must-Haves

- [ ] `RouteSegment` and `ViaPlacement` derive `Serialize`
- [ ] `generate_variants()` runs multiple configs sequentially, returning scored + serialized results
- [ ] Best variant auto-applied to world after generation
- [ ] `auto_route_variants()` WASM entry point returns JSON array
- [ ] Unit tests for variant types and default configs
- [ ] Integration test proves multi-variant generation on led_blink fixture
- [ ] WASM compiles (`cargo check -p cypcb-autoroute --target wasm32-unknown-unknown`)

## Verification

- `cargo test -p cypcb-autoroute --lib --release` — all existing + new unit tests pass
- `cargo test --test variant_generation --release` — integration test passes
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compiles clean
- `cargo check -p cypcb-render` — auto_route_variants() compiles
- `cargo check -p cypcb-router` — Serialize derives don't break downstream

## Observability Impact

- Signals added: `tracing::info!` per variant in `generate_variants()` (variant name, composite score, route count, via count); summary log with variant count + best name
- How a future agent inspects this: `RUST_LOG=cypcb_autoroute::variant=info cargo test --test variant_generation --release -- --nocapture`
- Failure state exposed: variant generation errors logged via `tracing::warn!`; individual variant routing failures captured in result (variant with empty routes or failed status)

## Inputs

- `crates/cypcb-autoroute/src/strategy.rs` — StrategyKind enum, RoutingStrategy trait
- `crates/cypcb-autoroute/src/scoring.rs` — score_board(), RoutingScore (already Serialize)
- `crates/cypcb-autoroute/src/lib.rs` — route_board(), AutorouteConfig, AutorouteParams
- `crates/cypcb-render/src/lib.rs` — auto_route() and auto_route_with_params() patterns for WASM bridge
- `crates/cypcb-router/src/types.rs` — RouteSegment, ViaPlacement, RoutingResult (need Serialize)
- S03 Forward Intelligence: route_board() returns RoutingResult, apply_routes() pipeline, both strategies produce valid results
- S04 Forward Intelligence: smoother always active, integrated into both strategies

## Expected Output

- `crates/cypcb-router/Cargo.toml` — serde dependency added
- `crates/cypcb-router/src/types.rs` — Serialize derives on RouteSegment, ViaPlacement, RoutingResult, RoutingStatus
- `crates/cypcb-autoroute/src/variant.rs` — New: VariantConfig, VariantResult, generate_variants(), default_variant_configs() + unit tests
- `crates/cypcb-autoroute/src/lib.rs` — Added `pub mod variant;`
- `crates/cypcb-render/src/lib.rs` — New `auto_route_variants()` method on PcbEngine
- `crates/cypcb-autoroute/tests/variant_generation.rs` — New: integration test (~100 LOC)
