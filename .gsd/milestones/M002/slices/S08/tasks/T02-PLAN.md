---
estimated_steps: 4
estimated_files: 5
---

# T02: Web perf verification, code duplication enforcement, quality gate extension

**Slice:** S08 — Performance & Polish
**Milestone:** M002

## Description

The remaining M002 performance targets (web load <3s on fast 3G, 3D viewer at 60fps) need automated verification. Code duplication threshold was deferred from S07 and needs tooling. All three checks get wired into `scripts/quality-gate.sh` as stages 7-8, making the quality gate the single source of truth for M002 completion criteria.

## Steps

1. **Playwright performance E2E spec.** Create `viewer/e2e/performance.spec.ts` with two tests: (a) Web load time — navigate to app, use `performance.timing` or `performance.getEntriesByType('navigation')` to measure domContentLoadedEventEnd - navigationStart. Assert <3000ms (localhost — fast 3G simulation not needed since the 132KB gzipped bundle transfers in ~660ms on 1.6Mbps). (b) 3D FPS — load app, wait for Ready, click 3D toggle, wait, read `window.__renderer3d.fps` via `page.evaluate()`, assert ≥30fps (headless WebGL may not hit 60fps consistently, so 30fps is the headless threshold). Run and verify both pass.

2. **Code duplication enforcement.** Install `jscpd` as a dev dependency in `viewer/`. Create `viewer/.jscpd.json` config: `{ "threshold": 0, "minLines": 10, "reporters": ["console"], "path": ["src/"], "ignore": ["**/*.test.ts", "**/__tests__/**"] }`. Run `npx jscpd` and verify zero duplicates. If any are found, evaluate — genuine duplication should be refactored, false positives should be excluded via config.

3. **Extend quality-gate.sh.** Add stage 7: autorouter performance benchmark (`cargo test --release -p cypcb-autoroute -- benchmark_500 --ignored`). Add stage 8: code duplication check (`cd viewer && npx jscpd --exitCode 1`). Update the stage counter from [N/6] to [N/8]. Ensure the script still fails fast on any stage failure.

4. **Full quality gate run.** Execute `./scripts/quality-gate.sh` end-to-end. All 8 stages must pass. Fix any issues found.

## Must-Haves

- [ ] Playwright `performance.spec.ts` verifying web load time and 3D FPS
- [ ] jscpd installed and configured with 0 threshold, 10-line minimum
- [ ] `scripts/quality-gate.sh` extended to 8 stages (perf benchmark + code duplication)
- [ ] Full quality gate passes all 8 stages

## Verification

- `cd viewer && npx playwright test e2e/performance.spec.ts` — both tests pass
- `cd viewer && npx jscpd src/ --min-lines 10 --threshold 0` — zero duplicates reported
- `./scripts/quality-gate.sh` — exits 0, all 8 stages show ✓

## Inputs

- `scripts/quality-gate.sh` — existing 6-stage gate from S07
- `viewer/playwright.config.ts` — existing Playwright config
- `viewer/src/renderer3d.ts` — `window.__renderer3d.fps` debug surface
- `viewer/package.json` — existing devDependencies
- T01 output: benchmark_500_component test exists and passes

## Expected Output

- `viewer/e2e/performance.spec.ts` — web load time + 3D FPS verification tests
- `viewer/.jscpd.json` — jscpd configuration
- `viewer/package.json` — jscpd added to devDependencies
- `scripts/quality-gate.sh` — updated to 8 stages
