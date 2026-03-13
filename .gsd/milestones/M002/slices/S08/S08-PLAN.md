# S08: Performance & Polish

**Goal:** Hit all M002 performance targets — autorouter <30s for 500 components, web load <3s, 3D at 60fps — and enforce code duplication threshold. Quality gate extended to cover performance benchmarks.
**Demo:** `./scripts/quality-gate.sh` passes all 8 stages (existing 6 + performance benchmarks + code duplication check). Synthetic 500-component board routes in <30s release.

## Must-Haves

- Autorouter routes a synthetic 500-component board in <30s (release build, `opt-level=3` per-crate)
- Adaptive grid resolution: coarser grid for larger boards to scale routing time sub-linearly
- Web load time verified <3s on fast 3G (1.6Mbps) via Playwright measurement
- 3D FPS verified ≥30fps via `window.__renderer3d.fps` debug surface in E2E test
- Code duplication threshold defined and enforced (0 exact duplicates >10 lines for TS via jscpd)
- `scripts/quality-gate.sh` extended with performance benchmark and code duplication stages
- All existing quality gate stages still pass

## Proof Level

- This slice proves: operational (performance meets quantitative targets)
- Real runtime required: yes (benchmarks require release builds and browser runtime)
- Human/UAT required: no

## Verification

- `cargo test --release -p cypcb-autoroute -- benchmark_500_component --ignored --nocapture` — prints timing, asserts <30s
- `cargo test --release -p cypcb-autoroute -- benchmark --ignored --nocapture` — existing benchmarks still pass
- `cd viewer && npx playwright test e2e/performance.spec.ts` — web load time <3s fast 3G, 3D FPS check
- `cd viewer && npx jscpd src/ --min-lines 10 --threshold 0` — zero duplicates >10 lines
- `./scripts/quality-gate.sh` — all 8 stages pass (exit 0)

## Observability / Diagnostics

- Runtime signals: benchmark test prints timing table with component count, net count, grid dimensions, routing time
- Inspection surfaces: `window.__renderer3d.fps` for 3D FPS, Playwright `performance.timing` for web load
- Failure visibility: benchmark test prints autorouter metrics on failure (grid size, routed/unrouted nets, time per net)

## Integration Closure

- Upstream surfaces consumed: `crates/cypcb-autoroute/` (A* router), `viewer/src/renderer3d.ts` (FPS surface), `scripts/quality-gate.sh` (gate script)
- New wiring introduced: per-crate opt-level overrides in `Cargo.toml`, performance E2E spec, jscpd config, quality gate stages 7-8
- What remains before milestone is truly usable end-to-end: nothing — S08 is the final slice

## Tasks

- [x] **T01: Autorouter performance — per-crate opt-level, adaptive grid, 500-component benchmark** `est:1h30m`
  - Why: Autorouter baseline is ~818ms for 8 components, linear extrapolation gives ~51s for 500 — over the 30s target. Per-crate opt-level=3 and adaptive grid resolution are the two highest-impact changes.
  - Files: `Cargo.toml`, `crates/cypcb-autoroute/src/lib.rs`, `crates/cypcb-autoroute/src/grid.rs`, `crates/cypcb-autoroute/tests/integration.rs`, `examples/synthetic-500.cypcb` (generated)
  - Do: (1) Add `[profile.release.package.cypcb-autoroute]` opt-level=3 and `[profile.release.package.pathfinding]` opt-level=3 to workspace Cargo.toml. (2) Add adaptive grid resolution to AutorouteConfig — scale resolution coarser for larger boards (e.g. double resolution for boards >80mm in either dimension). (3) Build a synthetic 500-component test board generator as a test helper. (4) Add `benchmark_500_component` test with timing assertion <30s. (5) Run existing benchmarks to verify no regression.
  - Verify: `cargo test --release -p cypcb-autoroute -- benchmark --ignored --nocapture` — 500-component routes in <30s, existing benchmarks still pass
  - Done when: 500-component benchmark test passes with <30s assertion in release mode

- [x] **T02: Web perf verification, code duplication enforcement, quality gate extension** `est:1h`
  - Why: Remaining M002 performance targets (web <3s, 3D 60fps) need verification tests, code duplication needs a threshold and tooling, and quality gate needs stages 7-8 for these checks.
  - Files: `viewer/e2e/performance.spec.ts`, `viewer/.jscpd.json`, `scripts/quality-gate.sh`, `viewer/package.json`
  - Do: (1) Add Playwright performance E2E spec that measures Time-to-Interactive via Performance API and checks `window.__renderer3d.fps` after 3D toggle. (2) Install jscpd, add `.jscpd.json` config with min-lines=10 threshold=0 for `viewer/src/`. (3) Extend quality-gate.sh with stage 7 (autorouter benchmark — `cargo test --release`) and stage 8 (code duplication — jscpd). (4) Run full quality gate to verify all 8 stages pass.
  - Verify: `./scripts/quality-gate.sh` exits 0 with all 8 stages passing
  - Done when: Full quality gate passes, performance E2E spec verifies web load <3s and 3D FPS ≥30

## Files Likely Touched

- `Cargo.toml` — per-crate profile overrides
- `crates/cypcb-autoroute/src/lib.rs` — adaptive grid resolution logic
- `crates/cypcb-autoroute/src/grid.rs` — grid resolution scaling
- `crates/cypcb-autoroute/tests/integration.rs` — synthetic board generator + 500-component benchmark
- `viewer/e2e/performance.spec.ts` — web load time + 3D FPS E2E tests
- `viewer/.jscpd.json` — jscpd configuration
- `viewer/package.json` — jscpd devDependency
- `scripts/quality-gate.sh` — stages 7-8
