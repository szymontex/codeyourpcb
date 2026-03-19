---
estimated_steps: 6
estimated_files: 7
---

# T01: Add AutorouteParams struct, parameterize routing engine and smoother, add WASM entry point

**Slice:** S05 — Realtime Tuning Parameters
**Milestone:** M004

## Description

Create the `AutorouteParams` struct as the user-facing parameter surface for tuning. Wire it through `AutorouteConfig` into cost functions (via cost, layer preference bias), grid resolution (density), and smoother chamfer aggressiveness (roundness). Add `auto_route_with_params(params_json)` WASM entry point in cypcb-render. Write integration tests proving different params produce different routing scores.

## Steps

1. **Define `AutorouteParams` in `lib.rs`** with `serde::Serialize` + `serde::Deserialize`:
   - `via_cost: f64` (0.1–10.0, default 1.0) — maps to `via_cost_multiplier`
   - `layer_preference: f64` (-1.0–1.0, default 0.0) — -1=bottom-heavy, 0=balanced, 1=top-heavy
   - `roundness: f64` (0.0–1.0, default 0.5) — controls chamfer aggressiveness
   - `density: f64` (0.5–2.0, default 1.0) — multiplier on auto-derived grid resolution
   - Add `AutorouteParams::default()` and `AutorouteParams::clamped()` that clamps all fields to valid ranges
   - Add `params: AutorouteParams` field to `AutorouteConfig` (default via `AutorouteParams::default()`)
   - Wire `config.params.via_cost` → `config.via_cost_multiplier` in `route_board()` (or have strategies read from params directly)
   - Wire `config.params.density` into `resolve_adaptive_grid_resolution()` as a final multiplier (1.0/density since higher density = finer grid = smaller resolution value), clamped so resolution never goes below 10µm

2. **Parameterize layer preference in `cost.rs`**:
   - Add `layer_preference: f64` field to `RoutingCost`
   - Update `RoutingCost::new()` to accept `layer_preference` parameter
   - In `neighbor_cost()`, replace the fixed `* 0.1` layer bias with: `cost += self.rules.layer_change_cost(to.2) * 0.1 * (1.0 + self.layer_preference * if to.2 == 0 { -1.0 } else { 1.0 })` — when layer_preference=1.0 (top-heavy), top layer (0) cost is reduced, bottom layer cost increased
   - Update all `RoutingCost::new()` call sites (pathfinder_v2.rs, astar_improved.rs, orchestrator.rs, existing tests) to pass the layer_preference from config

3. **Parameterize smoother roundness**:
   - Change `smooth_routes()` signature: add `roundness: f64` parameter
   - Pass roundness to `chamfer_corners()` (add parameter there too)
   - In `chamfer_corners()`: scale `max_chamfer` by roundness: `let max_chamfer = (Nm::from_mm(1.0).0 as f64 * roundness) as i64`; when roundness=0.0, max_chamfer=0 which skips chamfering (existing `< 1000` guard handles it)
   - Also scale divisor: `let chamfer_len = (len_a.min(len_b) / (3.0 / roundness.max(0.1)).ceil() as i64).min(max_chamfer)` — higher roundness = larger chamfer fraction
   - Actually simpler: `let chamfer_len = ((len_a.min(len_b) as f64 * roundness / 3.0) as i64).min(max_chamfer)` — roundness directly scales chamfer length
   - Update both strategy files' smoother calls to pass `config.params.roundness`

4. **Add `auto_route_with_params()` WASM entry point in `crates/cypcb-render/src/lib.rs`**:
   - New method: `pub fn auto_route_with_params(&mut self, params_json: String) -> String`
   - Deserialize `params_json` into `AutorouteParams` (return error JSON on failure)
   - Create `AutorouteConfig` with `.params` set from deserialized params, strategy=PathFinder
   - Call `route_board()` with this config (same pattern as existing `auto_route()`)
   - Return same JSON format as `auto_route()`

5. **Unit tests for AutorouteParams**:
   - `params_default_values` — verify defaults (1.0, 0.0, 0.5, 1.0)
   - `params_clamped` — verify out-of-range values get clamped
   - `params_serde_roundtrip` — serialize to JSON, deserialize back, verify equality
   - `params_from_json_partial` — deserialize JSON with missing fields, verify defaults fill in

6. **Integration test `tests/tuning_params.rs`**:
   - Route led_blink with default params → get baseline score
   - Route led_blink with `via_cost: 10.0` → score should have different (likely fewer) vias or different composite
   - Route led_blink with `roundness: 0.0` vs `roundness: 1.0` → smoothness should differ
   - Assert score differences are non-zero (params actually affect output)

## Must-Haves

- [ ] `AutorouteParams` struct with 4 fields, serde derives, Default impl, clamped() method
- [ ] `AutorouteConfig` has `params: AutorouteParams` field
- [ ] Via cost param flows through to `RoutingCost` via `via_cost_multiplier`
- [ ] Layer preference param affects layer change cost asymmetrically in `neighbor_cost()`
- [ ] Density param scales grid resolution in `resolve_adaptive_grid_resolution()`
- [ ] Roundness param controls chamfer aggressiveness in smoother
- [ ] `auto_route_with_params(params_json)` WASM entry point in cypcb-render
- [ ] Existing `auto_route()` unchanged and still works
- [ ] Integration test proves different params produce different scores

## Verification

- `cargo test -p cypcb-autoroute --lib --release` — all tests pass including new params tests
- `cargo test --test tuning_params --release` — integration test passes, different params produce different scores
- `cargo check -p cypcb-render --target wasm32-unknown-unknown` — WASM compiles clean
- `cargo test --test strategy_comparison --release -- led_blink` — existing comparison test still passes (backward compat)

## Observability Impact

- Signals added: `tracing::info!` in `auto_route_with_params` logging received params values
- How a future agent inspects this: `RUST_LOG=cypcb_render=info` shows params; integration test prints score comparison
- Failure state exposed: JSON error return from `auto_route_with_params` on bad input

## Inputs

- `crates/cypcb-autoroute/src/lib.rs` — current `AutorouteConfig` struct and `route_board()` entry point
- `crates/cypcb-autoroute/src/cost.rs` — `RoutingCost` with layer_change_cost `* 0.1` bias
- `crates/cypcb-autoroute/src/smoother.rs` — `smooth_routes()` and `chamfer_corners()` with hardcoded chamfer params
- `crates/cypcb-render/src/lib.rs` — `auto_route()` WASM method pattern
- S03-SUMMARY.md — PathFinder produces grid-aligned paths, config flows through strategies
- S04-SUMMARY.md — smoother always active, chamfer params ready to parameterize

## Expected Output

- `crates/cypcb-autoroute/src/lib.rs` — `AutorouteParams` struct + updated `AutorouteConfig`
- `crates/cypcb-autoroute/src/cost.rs` — `RoutingCost` with `layer_preference` field
- `crates/cypcb-autoroute/src/smoother.rs` — `smooth_routes()` + `chamfer_corners()` with roundness parameter
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — updated smoother call with roundness
- `crates/cypcb-autoroute/src/astar_improved.rs` — updated smoother call with roundness
- `crates/cypcb-render/src/lib.rs` — `auto_route_with_params()` method
- `crates/cypcb-autoroute/tests/tuning_params.rs` — integration test proving param influence on scores
