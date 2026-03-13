---
estimated_steps: 5
estimated_files: 6
---

# T03: Net Highlighting & WASM Mutation APIs

**Slice:** S06 — Competition Feature Parity & UI Polish
**Milestone:** M002

## Description

Two parallel concerns batched into one Rust build cycle: (1) net highlighting in the 2D renderer — click a trace to highlight its entire net with dimming of all other copper, (2) WASM-exposed mutation APIs for `rotate_component` and `set_board_size` that T04 will wire into UI. The Rust→WASM→TS pipeline for new mutations is the 4-file bottleneck (world.rs, lib.rs, wasm.ts interface, wasm.ts implementations) — doing both mutations in one pass is efficient.

## Steps

1. Add `highlightedNet: string | null` to `RenderState` in renderer.ts with `updateHighlightedNet()` spread function. In `drawTrace()`, when `highlightedNet` is set and trace doesn't match, draw with alpha 0.15 (dimmed). When trace matches, draw at full brightness with thicker glow. Same dimming logic for `drawPad()`.
2. Wire net highlighting in main.ts: on trace click (existing `onTraceSelect` handler), look up the trace's `net_name` from snapshot, set `highlightedNet`. Click empty space or press Escape → clear `highlightedNet` to null. Log `[Net] Highlighted: <name>` / `[Net] Cleared`.
3. Rust: add `rotate_component(&mut self, refdes: &str, delta_mdeg: i32) -> bool` to `BoardWorld` in world.rs. Use `find_by_refdes(refdes)` → `get_mut::<Rotation>(entity)` → add delta with normalization. Return false if refdes not found. Add unit test.
4. Rust: add `set_board_size(&mut self, width_nm: i64, height_nm: i64) -> bool` to `BoardWorld`. Find board entity, update `BoardSize` component. Return false if no board. Add unit test. Then expose both methods on WASM `PcbEngine` in `crates/cypcb-render/src/lib.rs`.
5. TypeScript: add `rotate_component(refdes: string, delta_mdeg: number): boolean` and `set_board_size(width_nm: number, height_nm: number): boolean` to `PcbEngine` interface in wasm.ts. Implement in `WasmPcbEngineAdapter` (delegate to WASM) and `MockPcbEngine` (update cached snapshot). Invalidate `cachedSnapshot` after both mutations.

## Must-Haves

- [ ] `highlightedNet` field in RenderState, dimming logic in drawTrace/drawPad
- [ ] Click-to-highlight and Escape-to-clear wired in main.ts
- [ ] `rotate_component()` on BoardWorld with unit test
- [ ] `set_board_size()` on BoardWorld with unit test
- [ ] Both methods exposed on WASM PcbEngine
- [ ] PcbEngine TS interface, WasmPcbEngineAdapter, and MockPcbEngine all updated

## Verification

- `cargo test -p cypcb-world` — passes including new rotation/resize tests
- `cargo test -p cypcb-render --all-features` — passes
- `cargo check -p cypcb-render --all-features` — compiles
- `cd viewer && npx tsc --noEmit` — zero TypeScript errors
- `grep -q "highlightedNet" viewer/src/renderer.ts` — net highlight present
- `grep -q "rotate_component" crates/cypcb-render/src/lib.rs` — WASM API present

## Observability Impact

- Signals added: `[Net] Highlighted: <name>` and `[Net] Cleared` console logs
- How a future agent inspects this: check `renderState.highlightedNet` value in closure; WASM mutation return values indicate success/failure
- Failure state exposed: `rotate_component`/`set_board_size` return false with console warning when entity not found

## Inputs

- `viewer/src/renderer.ts` — existing `drawTrace()` with `colorByNet` pattern to extend
- `viewer/src/main.ts` — existing `onTraceSelect` callback to hook into
- `crates/cypcb-world/src/world.rs` — `find_by_refdes()`, `get_mut::<T>()` patterns
- `crates/cypcb-render/src/lib.rs` — existing `add_trace_json()`/`remove_trace()` WASM export pattern
- `viewer/src/wasm.ts` — existing PcbEngine interface, adapter, and mock patterns

## Expected Output

- `viewer/src/renderer.ts` — modified with highlightedNet field and dimming logic
- `viewer/src/main.ts` — modified with highlight-on-click and Escape-to-clear
- `crates/cypcb-world/src/world.rs` — modified with rotate_component() and set_board_size()
- `crates/cypcb-render/src/lib.rs` — modified with WASM exports for both mutations
- `viewer/src/wasm.ts` — modified PcbEngine interface, adapter, and mock
