# T03: WASM Build & Integration

**Slice:** S02 — **Milestone:** M001

## Description

Build WASM module and integrate with TypeScript frontend.

Purpose: Connect the Rust PcbEngine to the JavaScript viewer. After this plan, the viewer can load .cypcb source and receive structured board data.

Output: Working WASM integration where TypeScript can call PcbEngine.load_source() and get_snapshot().

## Must-Haves

- [ ] "wasm-pack builds cypcb-render to viewer/pkg"
- [ ] "TypeScript can import and instantiate PcbEngine"
- [ ] "load_source returns error string or empty"
- [ ] "get_snapshot returns BoardSnapshot object"

## Files

- `viewer/src/wasm.ts`
- `viewer/package.json`
- `viewer/build-wasm.sh`
