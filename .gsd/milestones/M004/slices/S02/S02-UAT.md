# S02: Routing Quality Score System — UAT

**Milestone:** M004
**Written:** 2026-03-14

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: Scoring is a pure computation module — all outputs are deterministic numbers from deterministic inputs. No UI, no network, no user interaction. Unit tests + integration tests on real board files fully validate correctness.

## Preconditions

- Rust toolchain installed with `wasm32-unknown-unknown` target
- Working directory is the project root (`/workspace/codeyourpcb`)
- Test fixture files exist: `tests/fixtures/blink.cypcb`, `tests/fixtures/routing-test.cypcb`

## Smoke Test

Run `cargo test -p cypcb-autoroute --test scoring_integration` — all 4 tests pass, confirming scoring works end-to-end on real boards.

## Test Cases

### 1. All 7 Metrics Computed for Routed Board

1. Run `cargo test -p cypcb-autoroute --test scoring_integration -- score_routed_blink --nocapture`
2. Observe metric table in stderr output
3. **Expected:** All 7 fields present: total_length > 0 (Nm), via_count >= 0, drc_violations < 200, smoothness in [0.0, 1.0], crossings < 50, layer_balance in [0.0, 1.0], composite > 0.0

### 2. Simple Board Scoring

1. Run `cargo test -p cypcb-autoroute --test scoring_integration -- score_routed_routing_test --nocapture`
2. Observe metric table in stderr output
3. **Expected:** total_length > 0, via_count == 0 (simple 2-pad board doesn't need vias), drc_violations < 50, smoothness in [0.0, 1.0], crossings == 0 (simple board has no inter-net crossings), layer_balance == 1.0 (single-layer routing), composite > 0.0

### 3. Empty Board Produces Safe Defaults

1. Run `cargo test -p cypcb-autoroute --test scoring_integration -- score_empty_board_is_valid`
2. **Expected:** total_length == 0, via_count == 0, drc_violations == 0, smoothness == 1.0, crossings == 0, layer_balance == 1.0, composite == 0.0. No panics.

### 4. JSON Serialization Round-Trip

1. Run `cargo test -p cypcb-autoroute --test scoring_integration -- score_json_serialization`
2. **Expected:** RoutingScore serializes to JSON string containing all 7 field names: "total_length", "via_count", "drc_violations", "smoothness", "crossings", "layer_balance", "composite". Parsing back produces identical values.

### 5. Smoothness Angle Penalty Correctness

1. Run `cargo test -p cypcb-autoroute -- test_angle_penalty`
2. **Expected:** 0° → penalty 0.0, 45° → penalty 0.0, 90° → penalty 0.0, 22.5° → penalty 0.0 (nearest 45° multiple), 23° → penalty > 0 (not a 45° multiple), negative angles handled correctly.

### 6. Layer Balance Edge Cases

1. Run `cargo test -p cypcb-autoroute -- test_layer_balance`
2. **Expected:** Empty traces → 1.0, single layer → 1.0, perfectly balanced (equal counts per layer) → 1.0, imbalanced (e.g. 7:2) → ~0.2857.

### 7. Crossing Detection Filters

1. Run `cargo test -p cypcb-autoroute -- test_crossings`
2. **Expected:** Different-net same-layer intersecting segments → counted as crossing. Same-net intersections → NOT counted (these are junctions). Different-layer intersections → NOT counted (layers are independent).

### 8. Composite Formula Properties

1. Run `cargo test -p cypcb-autoroute -- test_composite`
2. **Expected:** All-zero metrics → composite 0.0. Adding trace length increases composite. Adding vias increases composite. Adding DRC violations increases composite significantly (×1000 weight). Lower composite always means better routing.

### 9. CLI Score Command Compiles

1. Run `cargo check -p cypcb-cli`
2. **Expected:** Clean compilation with no errors. Score subcommand registered in CLI.

### 10. WASM Compatibility

1. Run `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown`
2. **Expected:** Clean compilation — scoring module (including serde, DRC, spatial index) compiles for WASM target.

## Edge Cases

### Zero-Length Segments Skipped in Smoothness

1. Run `cargo test -p cypcb-autoroute -- test_smoothness_zero_length_segment_skipped`
2. **Expected:** Smoothness calculation skips segments with zero length (start == end) — avoids NaN/undefined atan2 results. Score is well-defined.

### Board With Traces But No Vias

1. Run `cargo test -p cypcb-autoroute --test scoring_integration -- score_routed_routing_test`
2. **Expected:** via_count == 0, all other metrics still compute correctly. Composite is lower than a board with vias (fewer penalties).

### Board With Vias

1. Run `cargo test -p cypcb-autoroute -- test_score_board_with_vias`
2. **Expected:** via_count > 0 reflected in score. Composite increases proportional to via count × weight.

## Failure Signals

- Any test in `cargo test -p cypcb-autoroute` fails — metric computation regression
- `scoring_integration` tests fail with assertion errors — baseline scores have drifted (routing algorithm changed)
- `cargo check -p cypcb-cli` fails — CLI dependency wiring broken
- `cargo check --target wasm32-unknown-unknown` fails — WASM-incompatible code introduced in scoring
- RoutingScore JSON output missing field names — serde derive broken
- Empty board produces NaN or panics — graceful default handling broken

## Requirements Proved By This UAT

- R103 (Routing Quality Scoring System) — all 7 metrics computed, composite formula validated, CLI outputs JSON breakdown, baseline scores established on real boards

## Not Proven By This UAT

- CLI `cypcb score <file>` end-to-end execution (requires pkg-config system deps not available in dev container)
- WASM `score()` binding for browser-side variant ranking (S06 scope)
- Score comparison across routing strategies (S07 scope — requires PathFinder from S03)
- Scoring on KiCad-parsed boards (requires S01→S03 pipeline integration)

## Notes for Tester

- DRC violation counts (50 for blink, 5 for routing-test) reflect current A*-based autorouter quality, not scoring bugs. These numbers should decrease when S03 (PathFinder) improves routing.
- Smoothness is 1.0 for all current routes because the postprocessor merges collinear segments — only after S04 (smoother) introduces non-grid angles will smoothness vary.
- Run integration tests with `--nocapture` to see full metric tables printed to stderr.
