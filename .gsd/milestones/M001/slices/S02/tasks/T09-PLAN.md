# T09: Enable Real WASM Integration

**Slice:** S02 — **Milestone:** M001

## Description

Enable real WASM integration by uncommenting the WASM import code in wasm.ts.

Purpose: With the WASM build now working (from 02-08), the viewer can use the real Rust-based PcbEngine instead of the JavaScript MockPcbEngine. This completes the WASM integration gap.

Output: viewer/src/wasm.ts uses real WASM module with MockPcbEngine as fallback only when WASM is unavailable.

## Must-Haves

- [ ] "TypeScript can import and instantiate real PcbEngine from WASM"
- [ ] "Real WASM PcbEngine replaces MockPcbEngine"

## Files

- `viewer/src/wasm.ts`
