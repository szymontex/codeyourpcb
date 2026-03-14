# T07: DRC Rendering Integration

**Slice:** S03 — **Milestone:** M001

## Description

Integrate DRC into the rendering pipeline so violations are computed on file load.

Purpose: Wire up the DRC engine to run automatically when a board is loaded, making violations available to the renderer. This enables DRC-05 real-time feedback.

Output: PcbEngine runs DRC after load_source(), violations included in BoardSnapshot.

## Must-Haves

- [ ] "DRC runs after board loads in WASM engine"
- [ ] "Violations are included in BoardSnapshot"
- [ ] "TypeScript types include violation data"

## Files

- `crates/cypcb-render/src/lib.rs`
- `crates/cypcb-render/src/snapshot.rs`
- `viewer/src/wasm.ts`
- `viewer/src/types.ts`
