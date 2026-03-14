---
estimated_steps: 8
estimated_files: 3
---

# T01: Build smoother and via optimizer modules with unit tests

**Slice:** S04 — Trace Smoother & Via Optimizer
**Milestone:** M004

## Description

Implement the two core algorithmic modules that convert raw grid-aligned autorouter output into clean 45°/90° traces and minimize unnecessary vias. The smoother operates on `Vec<RouteSegment>` (Nm coordinates) grouped by (net_id, layer), applying three passes: staircase-to-diagonal collapse, corner chamfering, and collinear segment merge. The via optimizer scans for eliminable via pairs. Both use `segment_distance()` from cypcb-drc for per-move DRC safety — if a smoothing move brings a segment too close to another net, that move is rejected.

## Steps

1. Create `crates/cypcb-autoroute/src/smoother.rs` with the public API: `smooth_routes(segments: &[RouteSegment], other_net_segments: &[RouteSegment], min_clearance: Nm) -> Vec<RouteSegment>`. Internal helper: `smooth_net_layer_group(group: &[RouteSegment], others: &[RouteSegment], min_clearance: Nm) -> Vec<RouteSegment>` that groups input by (net_id, layer) and smooths each group independently.

2. Implement **staircase-to-diagonal collapse**: scan for sequences of short segments alternating between two perpendicular directions (e.g., H-V-H-V). Replace the sequence with a single 45° diagonal + a short orthogonal segment to reach the endpoint. Verify the diagonal is DRC-clean against `other_net_segments` using `segment_distance()` before committing. If not DRC-clean, keep the original staircase.

3. Implement **corner chamfering**: for each remaining 90° bend (segment A ends where segment B starts, directions perpendicular), insert a short 45° chamfer segment. Chamfer length = `min(len_A, len_B) / 3` (capped at reasonable max). Verify DRC clearance on the chamfer segment.

4. Implement **collinear segment merge**: after the above passes, merge any consecutive segments with the same direction vector into a single segment. This cleans up fragments from chamfering.

5. Add angle enforcement validation: `is_valid_angle(start: Point, end: Point) -> bool` checks that the segment angle is a multiple of 45°. Assert this on all output segments. Use `atan2` with f64 cast for angle computation but check against exact integer direction patterns (dx==0, dy==0, |dx|==|dy|) to avoid floating-point ambiguity.

6. Create `crates/cypcb-autoroute/src/via_optimizer.rs` with: `optimize_vias(segments: Vec<RouteSegment>, vias: Vec<ViaPlacement>, other_net_segments: &[RouteSegment], min_clearance: Nm) -> (Vec<RouteSegment>, Vec<ViaPlacement>)`. Per-net: find via pairs (down-via at A, up-via at B) with a single segment between them on alternate layer. If a direct segment on the original layer from A→B is DRC-clean, eliminate both vias and replace with direct segment.

7. Add `pub mod smoother;` and `pub mod via_optimizer;` to `crates/cypcb-autoroute/src/lib.rs`.

8. Write unit tests (≥15): staircase collapse (3-step, 5-step, irregular), corner chamfer (90° bend, already-45° no-op), collinear merge, angle validation (valid/invalid), DRC rejection (move rejected when too close), via elimination (eliminable pair, non-eliminable pair), edge cases (empty input, single segment, zero-length segment), net_id/layer preservation.

## Must-Haves

- [ ] `smooth_routes()` produces segments with only 0°/45°/90°/135° angles
- [ ] Staircase patterns (alternating H/V steps) collapsed to diagonal + orthogonal
- [ ] 90° corners chamfered with 45° segments
- [ ] Collinear consecutive segments merged
- [ ] DRC safety: smoothed segments checked against other-net segments, move rejected if clearance violated
- [ ] `optimize_vias()` eliminates redundant via pairs when single-layer path is DRC-clean
- [ ] net_id, layer, and width preserved on all output segments
- [ ] No `std::time::Instant` or filesystem — WASM compatible
- [ ] ≥15 unit tests passing

## Verification

- `cargo test -p cypcb-autoroute --lib --release` — all unit tests pass including ≥15 new tests
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compiles without errors
- Manual inspection: unit test for staircase input (e.g., 10 alternating H/V steps) produces ≤3 output segments

## Inputs

- `crates/cypcb-router/src/types.rs` — `RouteSegment`, `ViaPlacement` structs (the data the smoother operates on)
- `crates/cypcb-drc/src/rules/clearance.rs` — `segment_distance(p1, p2, p3, p4) -> i64` for per-move clearance checking
- `crates/cypcb-autoroute/src/postprocess.rs` — existing collinear merge logic (reference, not reused — smoother operates on different data)
- S04-RESEARCH.md — algorithm details for staircase detection, chamfering, via optimization

## Expected Output

- `crates/cypcb-autoroute/src/smoother.rs` — ~300 LOC, smooth_routes() + internal helpers + ≥10 unit tests
- `crates/cypcb-autoroute/src/via_optimizer.rs` — ~150 LOC, optimize_vias() + ≥5 unit tests
- `crates/cypcb-autoroute/src/lib.rs` — 2 new mod declarations added

## Observability Impact

- **New signals:** `tracing::info!` in `smooth_routes()` logs before/after segment count per (net_id, layer) group; `tracing::debug!` logs each DRC-rejected smoothing move with segment coords and clearance distance
- **Inspection:** Future agents can verify smoother behavior by running `RUST_LOG=cypcb_autoroute::smoother=debug cargo test` and inspecting per-move rejection logs
- **Failure visibility:** DRC rejection unit test exercises the rejection path — if `segment_distance()` integration breaks, this test fails with a clear assertion on output segment count
