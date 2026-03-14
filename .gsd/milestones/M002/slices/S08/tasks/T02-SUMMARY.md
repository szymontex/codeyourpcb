---
id: T02
parent: S08
milestone: M002
provides:
  - Playwright performance E2E spec verifying web load <3s and 3D FPS ≥30
  - jscpd zero-duplication enforcement for viewer/src (0 threshold, 10-line minimum)
  - Quality gate extended to 8 stages (autorouter benchmark + code duplication)
  - Code deduplication — 7 clones removed via refactoring (geometry util, copper mesh helper, ratsnest helper, resize handler, handle positions)
key_files:
  - viewer/e2e/performance.spec.ts
  - viewer/.jscpd.json
  - viewer/src/geometry.ts
  - scripts/quality-gate.sh
key_decisions:
  - Headless WebGL FPS threshold set to 30fps (not 60fps) — headless Chromium WebGL varies by CI environment
  - Refactored all 7 jscpd clones instead of excluding them — genuine duplication eliminated through shared helpers
  - Used domContentLoadedEventEnd for load time measurement — correlates with WASM init completing
patterns_established:
  - Shared geometry utilities in viewer/src/geometry.ts — pointToSegmentDistance extracted from hit-test.ts and wasm.ts
  - addCopperMesh helper in renderer3d.ts for per-layer merged BufferGeometry creation
  - applyRoutesToSnapshot shared helper in wasm.ts for ratsnest regeneration
observability_surfaces:
  - Playwright perf spec logs domContentLoaded time and 3D FPS to test output
  - jscpd reports clone count and duplication percentage in console output
  - Quality gate stages 7-8 print pass/fail with labels
duration: 25m
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: Web perf verification, code duplication enforcement, quality gate extension

**Added Playwright performance E2E tests (load 164ms, 3D 60fps), enforced zero code duplication via jscpd with 7 clones refactored, and extended quality gate to 8 stages — all passing.**

## What Happened

Created `viewer/e2e/performance.spec.ts` with two tests: web load time measured via Navigation Timing API (164ms actual, <3000ms threshold) and 3D FPS via `window.__renderer3d.fps` after 3.5s render (60fps actual, ≥30fps threshold for headless).

Installed jscpd and ran it — found 7 genuine duplications (1.51%). Refactored all of them:
- Extracted `pointToSegmentDistance` from hit-test.ts and wasm.ts into shared `geometry.ts`
- Created `addCopperMesh` helper in renderer3d.ts to deduplicate 4 identical BufferGeometry+Material blocks
- Extracted `applyRoutesToSnapshot` in wasm.ts to deduplicate ratsnest regeneration between WasmPcbEngineAdapter and MockPcbEngine
- Extracted `computeHandlePositions` in renderer.ts to deduplicate resize handle position arrays
- Consolidated mouse/touch resize handlers in editor-panel.ts into shared `applyResize` and `stopDrag`

Extended quality-gate.sh from 6 to 8 stages: stage 7 (autorouter benchmark `benchmark_500`), stage 8 (jscpd duplication check). Also fixed cargo fmt issues inherited from T01's benchmark code.

## Verification

- `cd viewer && npx playwright test e2e/performance.spec.ts` — 2 passed (load 164ms, FPS 60)
- `cd viewer && npx jscpd src/ --min-lines 10 --threshold 0` — 0 clones found
- `cd viewer && npx vitest run` — 40 tests passed (refactoring didn't break anything)
- `./scripts/quality-gate.sh` — all 8 stages passed, exit 0

Slice-level verification:
- ✓ `cargo test --release -p cypcb-autoroute -- benchmark_500_component --ignored` — passes (0.04s)
- ✓ `cargo test --release -p cypcb-autoroute -- benchmark --ignored` — passes (T01)
- ✓ `cd viewer && npx playwright test e2e/performance.spec.ts` — load <3s, FPS ≥30
- ✓ `cd viewer && npx jscpd src/ --min-lines 10 --threshold 0` — zero duplicates
- ✓ `./scripts/quality-gate.sh` — all 8 stages pass

## Diagnostics

- Run `cd viewer && npx playwright test e2e/performance.spec.ts --reporter=list` to see perf numbers
- Run `cd viewer && npx jscpd src/ --min-lines 10 --threshold 0` to check for new duplication
- Run `./scripts/quality-gate.sh` to verify all 8 stages end-to-end

## Deviations

- Fixed cargo fmt issues in T01's benchmark code (autoroute lib.rs, integration tests) — fmt wasn't run during T01
- Refactored 7 code duplications rather than excluding them — plan said "evaluate and refactor genuine duplication, exclude false positives" but all 7 were genuine

## Known Issues

None.

## Files Created/Modified

- `viewer/e2e/performance.spec.ts` — Playwright E2E spec for web load time and 3D FPS verification
- `viewer/.jscpd.json` — jscpd configuration (0 threshold, 10-line minimum, src/ scope)
- `viewer/src/geometry.ts` — shared geometry utility (pointToSegmentDistance)
- `viewer/src/hit-test.ts` — import pointToSegmentDistance from geometry.ts
- `viewer/src/wasm.ts` — import geometry, extract applyRoutesToSnapshot, remove duplicate pointToSegmentDistance
- `viewer/src/renderer3d.ts` — addCopperMesh helper, replaced 4 duplicate mesh creation blocks
- `viewer/src/renderer.ts` — computeHandlePositions helper, deduplicated handle arrays
- `viewer/src/editor/editor-panel.ts` — shared applyResize/stopDrag for mouse+touch handlers
- `viewer/package.json` — jscpd added to devDependencies
- `scripts/quality-gate.sh` — updated from 6 to 8 stages
- `crates/cypcb-autoroute/src/lib.rs` — cargo fmt fix
- `crates/cypcb-autoroute/tests/integration.rs` — cargo fmt fix
