---
phase: 13-web-deployment
plan: 01
subsystem: build-pipeline
tags: [vite, wasm, wasm-opt, vite-plugin-wasm, web, production-build]

# Dependency graph
requires:
  - phase: 12-tauri-desktop-foundation
    provides: Conditional build targets (chrome105/safari13) in Vite config
provides:
  - Production-optimized WASM build pipeline with 29% size reduction (374KB → 264KB)
  - Vite config with WASM ES module support and top-level await
  - Cargo release profile optimized for size (opt-level=z, LTO, strip)
  - wasm-pack configuration with modern WASM features enabled
affects: [13-02, 13-03, 13-web-deployment]

# Tech tracking
tech-stack:
  added:
    - vite-plugin-wasm: ^3.5.0
    - vite-plugin-top-level-await: ^1.6.0
  patterns:
    - "Workspace-level release profile for size optimization"
    - "wasm-pack metadata configuration for aggressive optimization"
    - "Optional wasm-opt post-processing in build script"

key-files:
  created: []
  modified:
    - viewer/vite.config.ts
    - viewer/package.json
    - viewer/build-wasm.sh
    - Cargo.toml
    - crates/cypcb-render/Cargo.toml

key-decisions:
  - "Workspace-level release profile applies to all crates (not crate-specific)"
  - "wasm-opt flags configured in Cargo.toml to ensure wasm-pack uses them"
  - "Enabled bulk-memory and nontrapping-float-to-int for modern WASM features"
  - "SIMD not enabled to maintain browser compatibility"
  - "Post-build wasm-opt step optional (wasm-pack already runs it)"

patterns-established:
  - "build:web script for explicit web production builds"
  - "Vite rollupOptions.manualChunks separates vendor code"
  - "wasm-pack --release with size-optimized flags"

# Metrics
duration: 6m 54s
completed: 2026-01-30
---

# Phase 13 Plan 01: Web Production Build Optimization Summary

**WASM binary size reduced 29% (374KB → 264KB) via Cargo opt-level=z, LTO, strip, and wasm-opt -O4 with Vite WASM plugin integration**

## Performance

- **Duration:** 6m 54s
- **Started:** 2026-01-30T00:57:43Z
- **Completed:** 2026-01-30T01:04:33Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Vite production build configured with WASM ES module plugins for top-level await support
- WASM binary size optimization via workspace Cargo release profile (opt-level=z, LTO, strip)
- wasm-pack configured to use -O4 with modern WASM features (bulk-memory, nontrapping-float-to-int)
- Production build pipeline verified end-to-end (WASM + Vite + TypeScript)

## Task Commits

Each task was committed atomically:

1. **Task 1: Configure Vite for production WASM builds** - `3ec158c` (feat)
2. **Task 2: Optimize WASM binary size** - `af82028` (feat)

## Files Created/Modified
- `viewer/vite.config.ts` - Added vite-plugin-wasm and vite-plugin-top-level-await plugins, configured rollupOptions for vendor chunking
- `viewer/package.json` - Added WASM Vite plugins as devDependencies, explicit build:web script
- `viewer/build-wasm.sh` - Added explicit --release flag, wasm-opt post-processing with feature flags, size reporting
- `Cargo.toml` - Added workspace-level [profile.release] with size optimizations (opt-level=z, LTO, codegen-units=1, panic=abort, strip=true)
- `crates/cypcb-render/Cargo.toml` - Added [package.metadata.wasm-pack.profile.release] with -O4 and required WASM feature flags

## Decisions Made

**Workspace-level release profile:**
- Applied to Cargo.toml (workspace level) rather than cypcb-render specifically
- Ensures all crates benefit from size optimization (renderer, parser, world)
- opt-level=z chosen over opt-level=s for maximum size reduction

**wasm-pack configuration in Cargo.toml:**
- Configured wasm-opt flags directly via package.metadata to ensure wasm-pack uses them
- Required flags: --enable-bulk-memory, --enable-nontrapping-float-to-int for Rust 2024 WASM
- Did NOT enable --enable-simd per research (reduces browser compatibility)

**Post-build wasm-opt optional:**
- wasm-pack already runs wasm-opt with configured flags
- Script includes optional second pass for users with system-installed wasm-opt
- Not required since wasm-pack's built-in optimization achieves target size

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

**wasm-opt feature flags required:**
- Initial wasm-opt run failed with "Bulk memory operations require bulk memory" error
- Root cause: Modern Rust WASM uses bulk memory and saturating float conversions
- Resolution: Added --enable-bulk-memory and --enable-nontrapping-float-to-int to wasm-pack config
- Configured in Cargo.toml [package.metadata.wasm-pack.profile.release] so wasm-pack passes flags automatically

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**What's ready:**
- Production build pipeline works end-to-end (npm run build:web)
- WASM binary size optimized and well under 500KB target (264KB pre-compression)
- Vite correctly bundles WASM with ES module support
- Build output is static site deployable to any CDN

**Next steps:**
- Phase 13-02: Browser compatibility testing (Chrome, Firefox, Safari, Edge)
- Phase 13-03: CDN deployment configuration (likely Cloudflare Pages)
- WASM file needs to be copied to dist/ or served from pkg/ directory in deployment

**Known limitation:**
- WASM file built to viewer/pkg/ but not copied to viewer/dist/ during Vite build
- Current dynamic import `await import('../pkg/cypcb_render.js')` requires pkg/ to be accessible
- Deployment must serve both dist/ and pkg/ directories, or refactor import to bundle WASM in dist/

---
*Phase: 13-web-deployment*
*Completed: 2026-01-30*
