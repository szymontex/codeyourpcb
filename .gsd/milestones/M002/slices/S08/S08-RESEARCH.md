# S08: Performance & Polish — Research

**Date:** 2026-03-13

## Summary

S08 closes M002 by hitting the performance targets and landing deferred polish items. The codebase is in good shape — S07 left clean linters, tests, and quality gates. The main engineering challenges are: (1) the autorouter is too slow for the 500-component target even in release mode, (2) web load time needs measurement/validation rather than optimization (the bundle is already lean), and (3) several deferred items need integration without destabilizing the quality gate.

Current baselines (release, opt-level="z"): blink.cypcb (8 components, 7 nets) routes in 818ms. Linear extrapolation to 500 components gives ~51 seconds — over the 30s target. The release profile uses `opt-level="z"` (size-optimized) which significantly penalizes speed. Switching autorouter to `opt-level=3` via per-crate profile override and adding coarser grid resolution for larger boards are the two most impactful changes. The web load critical path is ~132KB gzipped (index + vendor + WASM), well within the 3s/3G target with streaming WASM compilation already in place. The 3D renderer already tracks FPS internally and is architected for 60fps (requestAnimationFrame loop, instanced meshes, merged geometries).

Three deferred items land here: board outline polygon editing (from S06), code duplication threshold enforcement (from S07), and incremental spatial index rebuild (from S03). Board outline polygon editing has the most scope risk — it ripples through export, DRC, 3D, and renderer. Recommend scoping it to convex polygons only (no cutouts) as a first step, or deferring entirely if it threatens the performance work.

## Recommendation

**Approach: Performance benchmarks first, then deferred items, polish last.**

1. **Autorouter performance**: Add per-crate `opt-level=3` override for cypcb-autoroute in workspace Cargo.toml. Generate a synthetic 500-component test board. Add benchmark test. Optimize grid resolution scaling (coarser grid for larger boards). Target: <30s release for 500 components.

2. **Web load time**: Add a Playwright-based load time test that measures Time-to-Interactive. The existing bundle is lean (132KB gzipped critical path). If it fails the 3s/3G budget, investigate: WASM preload hint (`<link rel="modulepreload">`), Monaco defer, font optimization.

3. **3D FPS verification**: Add a Playwright test that checks `window.__renderer3d.fps` after 3D toggle. The renderer already exposes this metric via debug surface.

4. **Code duplication**: Use `cargo-deny` for license/dependency dedup and a simple custom script for code pattern dedup (jscpd for TS, no mature Rust dedup tool exists). Define threshold: 0 exact duplicates >10 lines.

5. **Board outline polygon editing**: Scope carefully. Recommend rectangle-only with vertex drag (enhancing the existing 8-handle system) rather than full arbitrary polygon. Full polygon editing would ripple through gerber export (`outline.rs`), DRC edge clearance, 3D substrate geometry, and renderer. The risk/reward ratio is unfavorable for the last slice.

6. **Incremental spatial index**: Profile first — if `rebuild_spatial_index_full()` is <5ms, skip the optimization (it runs on mutation only, not per-frame).

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Rust benchmark harness | `criterion` crate | Statistical benchmarking with warmup, outlier detection, comparison baselines — std `#[bench]` is nightly-only |
| Code duplication detection (TS) | `jscpd` npm package | Established tool, supports TypeScript, configurable thresholds |
| Bundle size analysis | `rollup-plugin-visualizer` | Treemap visualization of Vite/Rollup bundles, zero-config |
| Web performance measurement | Playwright `page.metrics()` + Performance API | Already have Playwright infrastructure from S07, measures real load times |
| Per-crate opt-level | Cargo `[profile.release.package.X]` | Built into Cargo, no extra tooling |

## Existing Code and Patterns

- `crates/cypcb-autoroute/tests/integration.rs::benchmark_routing_time` — existing benchmark test (ignored), prints timing table. Extend this with 500-component board.
- `Cargo.toml [profile.release]` — workspace-wide `opt-level="z"`. Add per-crate overrides: `[profile.release.package.cypcb-autoroute]` with `opt-level=3` and `[profile.release.package.pathfinding]` with `opt-level=3`.
- `viewer/src/renderer3d.ts` — already tracks FPS via `currentFps` field, exposed through `window.__renderer3d.fps` debug surface. E2E test can read this.
- `viewer/src/wasm.ts::loadWasm()` — WASM init path. Uses `instantiateStreaming` already. Could benefit from `<link rel="preload" as="fetch" href="...wasm">` for parallel download.
- `viewer/src/renderer.ts::drawResizeHandles()` + `hitTestResizeHandle()` — existing resize handle infrastructure from S06. Polygon editing would extend this.
- `scripts/quality-gate.sh` — 6-stage gate. Performance benchmarks and dedup checks would become stages 7-8.
- `crates/cypcb-render/src/lib.rs::rebuild_spatial_index_full()` — called 4 times on various mutations. Profile before optimizing.
- `crates/cypcb-autoroute/src/lib.rs::AutorouteConfig` — `grid_resolution_nm` is configurable. Adaptive resolution (coarser for larger boards) is a straightforward change.
- `crates/cypcb-export/src/gerber/outline.rs` — exports board outline as closed polygon. Currently assumes rectangle from `BoardInfo`. Polygon outlines would require changing the data model.

## Constraints

- **opt-level="z" is necessary for WASM bundle size** — cannot change workspace-wide. Must use per-crate overrides for autorouter speed.
- **Autorouter is not in the WASM bundle** — cypcb-autoroute is not a dependency of cypcb-render. Autorouter runs server-side or would need a separate WASM build for in-browser routing.
- **No X server in build environment** — 3D FPS testing must use headless Chromium with debug surface checks (as established in S07).
- **Desktop crates excluded from quality gates** — missing pkg-config/gio-2.0 system deps. DESK-10 (<1s startup) cannot be verified in this environment.
- **Quality gate must keep passing** — any new stages must not break the existing 6-stage pipeline.
- **Existing E2E tests are headless Chromium only** — WebGL rendering varies in headless, so FPS test should use the `window.__renderer3d.fps` debug surface.
- **blink.cypcb has only 8 components** — need synthetic test board generation for 500-component benchmark.
- **Board outline is currently always rectangular** — `BoardInfo` has `width_nm`/`height_nm` fields only. Polygon outlines require schema change in `cypcb-core` or `cypcb-world`.

## Common Pitfalls

- **Benchmarking in debug mode** — debug builds are 2-10x slower than release. All performance baselines must use `--release`. The existing `benchmark_routing_time` test correctly uses release but is gated behind `#[ignore]`.
- **opt-level="z" vs "3" confusion** — "z" optimizes for binary size (important for WASM), "3" optimizes for speed. The autorouter needs speed, the WASM render needs size. Per-crate profiles solve this cleanly.
- **Grid resolution scaling** — coarser grids route faster but produce lower quality results (stairstepping, suboptimal clearance usage). Need to verify output quality doesn't degrade unacceptably at coarser resolution.
- **Synthetic board generation** — 500 components with realistic net connectivity is non-trivial. Random placement + random nets may not represent real board routing difficulty. Use a structured generation approach (grid of components with nearest-neighbor nets).
- **Board outline polygon scope creep** — full polygon editing touches export (gerber outline, DRC edge clearance), 3D rendering (substrate geometry), and the data model. Scope to minimal viable: vertex-drag on existing rectangle handles, or skip entirely if it threatens performance work.
- **Monaco language workers in bundle size** — `ts.worker` is 6.7MB, `css.worker` is 1MB. These are lazy-loaded Monaco workers, not on the critical path, but they inflate the total `dist/` to 15MB. Not a problem for load time but worth noting for CDN costs.

## Open Risks

- **500-component routing may hit memory limits in WASM** — grid for a 100x100mm board at 63µm resolution is ~2.5M cells per layer. With u8 per cell and 2 layers, that's ~5MB. Fine for native, but WASM memory limits could matter. Not blocking since autorouter isn't in WASM bundle yet, but relevant for future.
- **A* pathfinding on large grids may have algorithmic bottleneck** — the `pathfinding` crate's A* is generic but may not be optimal for grid-based routing. If per-crate opt-level + coarser grid don't hit 30s, may need to profile and optimize the hot path (cost function, neighbor generation).
- **Board outline polygon editing may not be achievable in this slice** — it's the riskiest deferred item. If it threatens the performance work, recommend cutting it and documenting as a follow-up. The rectangular outline with 8 resize handles from S06 is fully functional.
- **3G load time definition varies** — "3G" ranges from 400kbps (slow 3G) to 1.6Mbps (fast 3G). Need to define which 3G profile we're targeting. Recommend fast 3G (1.6Mbps) as that's what Lighthouse uses.

## Requirements Supported

| Requirement | Contribution |
|-------------|-------------|
| **WEB-01** — Web app loads in <3s on 3G | Primary — measure and verify load time |
| **DESK-10** — Desktop starts in <1s | Measurement only — cannot verify without desktop env |
| **Milestone criterion** — autorouter <30s for 500 components | Primary — benchmark + optimize |
| **Milestone criterion** — 3D viewer at 60fps | Verify via debug surface |
| **Milestone criterion** — zero code duplication above threshold | Define threshold + enforce |
| **Milestone criterion** — all linters pass | Maintained from S07 |

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Web performance | `sickn33/antigravity-awesome-skills@web-performance-optimization` (1.1K installs) | Available — covers web vitals, bundle optimization |
| Rust performance | None found specific to Rust perf optimization | N/A |
| General perf | `supercent-io/skills-template@performance-optimization` (10.6K installs) | Available — generic, likely not deeply relevant |

No skills are directly relevant enough to recommend installing. The work is domain-specific (PCB autorouter + Vite/WASM bundle optimization) and covered by existing codebase patterns.

## Sources

- Autorouter baseline: `cargo test --release -p cypcb-autoroute -- benchmark --ignored` → 818ms for 8-component blink.cypcb
- Bundle sizes: `viewer/dist/assets/` inspection → 132KB gzipped critical path (index + vendor + WASM)
- WASM init: `viewer/pkg/cypcb_render.js` uses `WebAssembly.instantiateStreaming` (optimal)
- 3D FPS debug surface: `viewer/src/renderer3d.ts` exposes `window.__renderer3d.fps`
- Deferred items: S06-SUMMARY.md (polygon editing), S07-SUMMARY.md (code duplication), S03-ASSESSMENT.md (incremental spatial index)
- Feature matrix gap list: `docs/competition-feature-matrix.md` — Priority 3 items inform polish direction
- Per-crate Cargo profiles: [Cargo Reference — Profile Overrides](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides)
