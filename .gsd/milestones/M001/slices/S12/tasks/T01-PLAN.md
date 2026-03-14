# T01: Production Build Pipeline

**Slice:** S12 — **Milestone:** M001

## Description

Optimize the production build pipeline for web deployment: configure Vite with WASM plugins, optimize WASM binary size, and set Cargo release profile for minimal WASM output.

Purpose: WEB-01 requires <3s load on 3G. WASM optimization is the critical path — unoptimized builds are 2-5x larger.
Output: Production build pipeline producing optimized assets ready for CDN deployment.

## Must-Haves

- [ ] "Production build completes without errors"
- [ ] "WASM binary is optimized for size (wasm-opt applied)"
- [ ] "Vite config supports WASM ES modules with top-level await"
- [ ] "Build output works in Chrome, Firefox, Safari, Edge"

## Files

- `viewer/vite.config.ts`
- `viewer/build-wasm.sh`
- `viewer/package.json`
- `crates/cypcb-render/Cargo.toml`
