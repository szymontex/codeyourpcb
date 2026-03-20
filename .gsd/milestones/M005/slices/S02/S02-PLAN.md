# S02: Routing Quality — 0 Unrouted on Blink LED

**Goal:** PathFinder routes all 25 connections on Blink LED with 0 unrouted, proven by cargo test and WASM binary rebuilt with the fix.
**Demo:** `cargo test --release -p cypcb-autoroute -- route_blink_board` prints `RoutingStatus::Complete` with 0 unrouted. New `test_blink_led_zero_unrouted` explicitly asserts `unrouted == 0`. WASM binary in `viewer/pkg/` contains the fix.

## Must-Haves

- Rip-up ghost cell bug in `pathfinder_v2.rs` fixed — no more `mark_route(u32::MAX)` poisoning the grid
- Existing `route_blink_board` test passes with `RoutingStatus::Complete`
- New `test_blink_led_zero_unrouted` test asserts `unrouted == 0`, segments > 0, vias < 20
- Full `cargo test --release -p cypcb-autoroute` suite passes (no regressions)
- WASM binary rebuilt with the fix (`viewer/pkg/cypcb_render_bg.wasm`)

## Proof Level

- This slice proves: contract
- Real runtime required: no (cargo test is sufficient; WASM rebuild is for downstream slices)
- Human/UAT required: no

## Verification

- `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture` — must show `RoutingStatus::Complete`
- `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` — must show `unrouted_count == 0`, segments > 0, vias < 20
- `cargo test --release -p cypcb-autoroute` — full suite, no failures
- `viewer/pkg/cypcb_render_bg.wasm` exists and is newer than the fix commit

## Integration Closure

- Upstream surfaces consumed: `pathfinder_v2.rs` (bug site), `grid.rs` (`clear_route()` API — unchanged), `integration.rs` (existing test)
- New wiring introduced in this slice: none (pure bug fix + test addition)
- What remains before the milestone is truly usable end-to-end: S03 (E2E tests consuming the 0-unrouted guarantee), S04 (variant generation via Worker)

## Tasks

- [x] **T01: Fix rip-up ghost cell bug in PathFinder** `est:30m`
  - Why: The root cause of 5/25 unrouted connections — `mark_route(x, y, layer, u32::MAX)` in the rip-up loop corrupts the grid, making `clear_route(net_id)` a no-op. Ghost trace cells accumulate as invisible obstacles.
  - Files: `crates/cypcb-autoroute/src/pathfinder_v2.rs`
  - Do: Remove the `for &(x, y, layer) in &cells { grid.mark_route(x, y, layer as usize, u32::MAX); }` loop from the rip-up block (around line 255). Keep `congestion_map.unmark_net(&cells)` and `grid.clear_route(net_id)` — those are correct. Run the existing `route_blink_board` test to verify the fix achieves `RoutingStatus::Complete`.
  - Verify: `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture`
  - Done when: `route_blink_board` test passes with `RoutingStatus::Complete` (all 7 nets routed, 0 unrouted)

- [x] **T02: Add zero-unrouted proof test and rebuild WASM** `est:30m`
  - Why: The S02→S03 boundary artifact — an explicit test asserting `unrouted == 0` with detailed metrics. Plus WASM rebuild so the fix is available in browser for S03 E2E tests.
  - Files: `crates/cypcb-autoroute/tests/integration.rs`, `viewer/pkg/cypcb_render_bg.wasm`
  - Do: Add `test_blink_led_zero_unrouted` test to `integration.rs` asserting: `unrouted_count == 0` (from RoutingStatus), `routes.len() > 0`, `via_count < 20`, timing < 60s. Run full autoroute test suite for regression check. Then rebuild WASM via `viewer/build-wasm.sh`.
  - Verify: `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` passes; `cargo test --release -p cypcb-autoroute` full suite passes; `viewer/pkg/cypcb_render_bg.wasm` exists
  - Done when: New test passes asserting 0 unrouted, full suite green, WASM binary rebuilt

## Observability / Diagnostics

- **Runtime signals:** The `route_blink_board` test prints a per-iteration metrics table showing `routed / unrouted / total` counts. After the fix, iteration output should converge to 0 unrouted (previously plateaued at 5/25). `test_blink_led_zero_unrouted` (T02) will assert `unrouted_count == 0` as a hard contract.
- **Inspection surfaces:** `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture` is the primary diagnostic — it dumps iteration-level routing progress. Grep for `Unrouted` or `RoutingStatus` in output to assess routing health.
- **Failure visibility:** If ghost cells re-emerge (regression), the `route_blink_board` test fails with `RoutingStatus::Partial` and the metrics table shows which nets remain unrouted. The `test_blink_led_zero_unrouted` test (T02) will fail with explicit `unrouted_count != 0`.
- **Redaction:** No secrets or PII involved — all diagnostics are safe to log.

## Files Likely Touched

- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — bug fix (remove ~3 lines)
- `crates/cypcb-autoroute/tests/integration.rs` — new test_blink_led_zero_unrouted
- `viewer/pkg/cypcb_render_bg.wasm` — rebuilt binary
