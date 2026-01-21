---
phase: 02-rendering
plan: 08
subsystem: wasm
tags: [wasm, build, feature-flags, tree-sitter]
dependency-graph:
  requires: [02-01, 02-03]
  provides: ["wasm-build", "viewer-pkg-artifacts"]
  affects: [03-validation]
tech-stack:
  added: []
  patterns: ["feature-flag-conditional-compilation", "wasm-pack-web-target"]
key-files:
  created:
    - viewer/test-wasm.mjs
  modified:
    - Cargo.toml
    - crates/cypcb-parser/Cargo.toml
    - crates/cypcb-parser/build.rs
    - crates/cypcb-parser/src/lib.rs
    - crates/cypcb-world/Cargo.toml
    - crates/cypcb-world/src/lib.rs
    - crates/cypcb-render/Cargo.toml
    - crates/cypcb-render/src/lib.rs
    - crates/cypcb-render/src/snapshot.rs
    - viewer/build-wasm.sh
decisions:
  - id: tree-sitter-feature-flag
    choice: "Conditional compilation via feature flags"
    rationale: "tree-sitter requires C compilation which doesn't work for wasm32-unknown-unknown target"
  - id: split-impl-blocks
    choice: "Separate WASM-exposed and internal impl blocks"
    rationale: "wasm_bindgen on impl block exposes all methods; split prevents non-WASM types from being exported"
  - id: snapshot-deserialize
    choice: "Add Deserialize to BoardSnapshot types"
    rationale: "WASM mode receives pre-parsed JSON from JavaScript"
metrics:
  duration: "45 minutes"
  completed: "2026-01-21"
---

# Phase 02 Plan 08: Fix WASM Build Summary

## One-liner

Feature flags for conditional tree-sitter compilation enabling successful wasm-pack builds.

## What Was Done

### Task 1: Configure getrandom features for WASM

Added feature flags throughout the crate hierarchy to exclude tree-sitter from WASM builds:

**cypcb-parser/Cargo.toml:**
- Added `tree-sitter-parser` feature (default enabled)
- Made `tree-sitter` and `cc` dependencies optional, gated behind the feature

**cypcb-parser/build.rs:**
- Wrapped C compilation in `#[cfg(feature = "tree-sitter-parser")]`
- No-op build when feature is disabled

**cypcb-parser/src/lib.rs:**
- Conditionally compiled parser module, language(), node_kinds
- AST types and error types always available (no tree-sitter dependency)

**cypcb-world/Cargo.toml:**
- Added `sync` feature (default enabled) that enables `cypcb-parser/tree-sitter-parser`
- Sync module only available when parsing is needed

**cypcb-world/src/lib.rs:**
- Wrapped sync module in `#[cfg(feature = "sync")]`

**cypcb-render/Cargo.toml:**
- Added `native` feature (default) for full parsing support
- Added `wasm` feature for tree-sitter-free builds
- Dependencies use `default-features = false`

**cypcb-render/src/lib.rs:**
- Split impl blocks: WASM-exposed methods in one, internal methods in another
- Added `load_snapshot()` for WASM mode (receives pre-parsed JSON)
- Native `load_source()` only available with native feature

**workspace Cargo.toml:**
- Set `default-features = false` for cypcb-parser and cypcb-world

### Task 2: Run wasm-pack build and verify artifacts

**viewer/build-wasm.sh:**
- Updated to use `--no-default-features --features wasm`
- Added `GLIBC_TUNABLES=glibc.rtld.optional_static_tls=2048` for Linux TLS issue
- Removed error fallback messaging

Build produces:
- `viewer/pkg/cypcb_render_bg.wasm` (240KB)
- `viewer/pkg/cypcb_render.js` (JavaScript bindings)
- `viewer/pkg/cypcb_render.d.ts` (TypeScript types)
- `viewer/pkg/package.json`

### Task 3: Verify WASM module loads in Node.js

**viewer/test-wasm.mjs:**
- Loads WASM bytes directly from file (Node.js doesn't support fetch for local files)
- Creates PcbEngine instance
- Tests load_snapshot() with a BoardSnapshot object
- Verifies get_snapshot() returns expected data
- Tests query_point() method
- Confirms footprint library integration (0402 pads returned)

## Commits

| Commit | Description |
|--------|-------------|
| 2b1366e | feat(02-08): configure WASM build with feature flags |
| ff923f2 | feat(02-08): update build-wasm.sh for successful WASM builds |
| 57a9151 | test(02-08): add WASM smoke test for Node.js verification |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] TLS allocation error on Linux**
- **Found during:** Task 2
- **Issue:** `cannot allocate memory in static TLS block` error from libmiette_derive
- **Fix:** Added `GLIBC_TUNABLES=glibc.rtld.optional_static_tls=2048` environment variable
- **Files modified:** viewer/build-wasm.sh

**2. [Rule 1 - Bug] wasm_bindgen on impl block exports all methods**
- **Found during:** Task 1
- **Issue:** Methods returning BoardSnapshot caused `WasmDescribe` not satisfied errors
- **Fix:** Split impl blocks into WASM-exposed and internal (non-exported)
- **Files modified:** crates/cypcb-render/src/lib.rs

## Technical Notes

### Architecture Decision: Parsing in JavaScript for WASM

The WASM module does NOT include tree-sitter parsing because:
1. tree-sitter requires C code compilation via `cc` crate
2. Standard C toolchains can't cross-compile to wasm32-unknown-unknown
3. Emscripten would add complexity without benefit

Instead, the architecture is:
- **JavaScript handles parsing** - The existing MockPcbEngine in main.ts parses .cypcb syntax
- **WASM receives pre-parsed data** - `load_snapshot()` accepts a BoardSnapshot JS object
- **WASM provides ECS world** - Footprint library, spatial queries, component management

This separation of concerns is cleaner than trying to compile tree-sitter to WASM.

### Feature Flag Matrix

| Crate | Feature | Default | Effect |
|-------|---------|---------|--------|
| cypcb-parser | tree-sitter-parser | yes | Enables tree-sitter parsing |
| cypcb-world | sync | yes | Enables AST-to-ECS sync |
| cypcb-render | native | yes | Full parsing support |
| cypcb-render | wasm | no | Tree-sitter-free WASM build |

## Verification Results

1. **WASM Build**: `./viewer/build-wasm.sh` exits with code 0
2. **Artifacts Exist**: `viewer/pkg/cypcb_render_bg.wasm` present (240KB)
3. **Module Valid**: `node test-wasm.mjs` passes all checks
4. **No Regressions**: All 246+ Rust tests still pass

## Next Phase Readiness

Phase 2 Rendering is now complete with gap #1 (WASM build) closed.

Ready to proceed to Phase 3 (Validation) which will add:
- Design rule checking
- Error highlighting
- Real-time validation feedback in the viewer
