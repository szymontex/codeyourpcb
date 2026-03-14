# T02: Frontend Scaffolding

**Slice:** S02 — **Milestone:** M001

## Description

Create minimal frontend scaffolding for the PCB viewer web application.

Purpose: Establish the TypeScript/Vite project structure that will load the WASM module and render to canvas. This runs in parallel with WASM crate setup.

Output: Working dev server with HTML canvas and TypeScript infrastructure ready for WASM integration.

## Must-Haves

- [ ] "Dev server starts and serves HTML page"
- [ ] "TypeScript compiles without errors"
- [ ] "WASM module loading code exists (even if module not ready)"

## Files

- `viewer/index.html`
- `viewer/package.json`
- `viewer/tsconfig.json`
- `viewer/src/main.ts`
- `viewer/src/wasm.ts`
- `viewer/vite.config.ts`
