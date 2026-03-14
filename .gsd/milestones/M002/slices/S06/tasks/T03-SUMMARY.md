---
id: T03
parent: S06
milestone: M002
provides:
  - Net highlighting in 2D renderer with click-to-highlight and Escape-to-clear
  - rotate_component() and set_board_size() on BoardWorld with unit tests
  - Both mutations exposed on WASM PcbEngine (lib.rs)
  - PcbEngine TS interface, WasmPcbEngineAdapter, and MockPcbEngine updated with rotate_component and set_board_size
key_files:
  - viewer/src/renderer.ts
  - viewer/src/main.ts
  - crates/cypcb-world/src/world.rs
  - crates/cypcb-render/src/lib.rs
  - viewer/src/wasm.ts
key_decisions:
  - Pad dimming uses global alpha 0.15 when net is highlighted (pads don't carry net info in snapshot, so all pads dim rather than per-pad net matching)
  - Net highlight glow uses 2.0x width at 0.3 alpha (slightly subtler than selection glow at 2.5x/0.35)
  - WASM set_board_size uses BigInt for i64 params, rotate_component uses plain number for i32
patterns_established:
  - Net highlighting pattern — highlightedNet field in RenderState, dimming in drawTrace/drawPad via colorWithAlpha(0.15)
  - BoardWorld mutation API pattern — methods return bool for success/failure, normalize values internally
observability_surfaces:
  - Console logs: `[Net] Highlighted: <name>` and `[Net] Cleared`
  - rotate_component/set_board_size return false with console warning when entity not found (MockPcbEngine)
  - renderState.highlightedNet readable in render loop closure
duration: 1 context window
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T03: Net Highlighting & WASM Mutation APIs

**Added net-level trace highlighting with click/Escape toggle, plus rotate_component and set_board_size mutations across Rust→WASM→TS pipeline.**

## What Happened

Implemented two parallel concerns in one pass through the 4-file WASM pipeline:

1. **Net highlighting** — Added `highlightedNet: string | null` to RenderState. In `drawTrace()`, when a net is highlighted, non-matching traces render at alpha 0.15 (dimmed) while matching traces get a glow effect (2x width, 0.3 alpha) and 15% brightness boost. Pads dim globally since they don't carry net info in the snapshot. In main.ts, clicking a trace sets `highlightedNet` to the trace's net_name; clicking empty space or pressing Escape clears it.

2. **WASM mutations** — Added `rotate_component(refdes, delta_mdeg)` and `set_board_size(width_nm, height_nm)` to BoardWorld in world.rs. Both return bool for success/failure. Rotation normalizes to [0, 360000) using rem_euclid. Both exposed on PcbEngine in lib.rs, then mirrored in the TS PcbEngine interface, WasmPcbEngineAdapter (delegates to WASM with BigInt conversion), and MockPcbEngine (updates cached snapshot directly). Both invalidate cachedSnapshot after successful mutation.

## Verification

- `cargo test -p cypcb-world` — 135 passed, 1 pre-existing failure (test_sync_named_pin, unrelated). New tests test_rotate_component and test_set_board_size both pass.
- `cargo test -p cypcb-render --all-features` — 32 passed, 0 failed
- `cargo check -p cypcb-render --all-features` — compiles (1 pre-existing warning)
- `cd viewer && npx tsc --noEmit` — zero TypeScript errors
- `cd viewer && npx vite build` — build succeeds
- `grep -q "highlightedNet" viewer/src/renderer.ts` — PASS
- `grep -q "rotate_component" crates/cypcb-render/src/lib.rs` — PASS
- Slice-level checks: all 10 pass

## Diagnostics

- Check `renderState.highlightedNet` in render loop to see current highlight state
- Console: `[Net] Highlighted: VCC` / `[Net] Cleared` on click/escape
- WASM mutations: return value indicates success (true) or entity-not-found (false)
- MockPcbEngine logs: `[MockEngine] rotate_component: R1 → 90°`, `[MockEngine] set_board_size: 100mm x 80mm`

## Deviations

None.

## Known Issues

- Pre-existing test failure: `sync::tests::test_sync_named_pin` in cypcb-world (not related to this task)
- Pad dimming is global (all pads dim when any net is highlighted) because PadInfo doesn't carry net association. A future improvement could look up pad-to-net mapping from the snapshot's net connections.

## Files Created/Modified

- `viewer/src/renderer.ts` — Added highlightedNet to RenderState, dimming logic in drawTrace/drawPad, updateHighlightedNet() helper
- `viewer/src/main.ts` — Wired highlight-on-click in onTraceSelect, Escape-to-clear, highlightedNet in render state
- `crates/cypcb-world/src/world.rs` — Added rotate_component() and set_board_size() with unit tests
- `crates/cypcb-render/src/lib.rs` — Exposed rotate_component() and set_board_size() on WASM PcbEngine
- `viewer/src/wasm.ts` — Updated PcbEngine interface, WasmPcbEngineAdapter, and MockPcbEngine with both mutations
