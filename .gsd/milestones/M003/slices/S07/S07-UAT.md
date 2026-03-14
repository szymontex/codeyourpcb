# S07: Polish, Bugs & Verification — UAT

**Milestone:** M003
**Written:** 2026-03-13

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: S07 is a polish/verification slice — all changes are testable via automated quality gate, version grep, and E2E tests. No new user-facing features requiring human-experience testing.

## Preconditions

- `cd /workspace/codeyourpcb`
- Node.js, Rust toolchain, and Playwright browsers available
- Dev server not required (Playwright launches its own via webServer config)

## Smoke Test

Run `bash scripts/quality-gate.sh` — all 8 stages should pass in ~40s. If this passes, the slice is functionally complete.

## Test Cases

### 1. Quality gate passes all 8 stages

1. Run `bash scripts/quality-gate.sh`
2. **Expected:** All 8 stages print `✓` and final line is `=== All stages passed ===`. No stage shows errors or non-zero exit.

### 2. Version strings are 0.1.0-beta

1. Run `grep '"version"' viewer/package.json src-tauri/tauri.conf.json`
2. Run `grep '^version' Cargo.toml | head -1`
3. **Expected:** All three show `0.1.0-beta`.

### 3. ESLint passes with zero errors

1. Run `cd viewer && npx eslint src/`
2. **Expected:** Exit code 0, no output (clean).

### 4. jscpd finds zero clones

1. Run `cd viewer && npx jscpd --exitCode 1`
2. **Expected:** Exit code 0, "Found 0 clones" in output, 0% duplication.

### 5. Vitest unit tests pass

1. Run `cd viewer && npx vitest run`
2. **Expected:** 127 tests passed, 11 suites, 0 failures.

### 6. Playwright E2E tests pass

1. Run `cd viewer && npx playwright test`
2. **Expected:** 94 tests passed, 0 failures.

### 7. JLCPCB error handling — HTTP error vs empty results

1. Run `cd viewer && npx vitest run -- --grep "JLCPCB"`
2. **Expected:** JLCPCB test suite passes including tests for HTTP error throwing and empty result handling.

### 8. Prefs-theme single-click E2E test

1. Run `cd viewer && npx playwright test e2e/theme.spec.ts`
2. **Expected:** 6 tests pass including "Preferences modal theme button cycles theme with single click".

### 9. errors.spec.ts stability

1. Run `cd viewer && for i in 1 2 3 4 5; do npx playwright test e2e/errors.spec.ts --reporter=line 2>&1 | tail -1; done`
2. **Expected:** All 5 runs show "5 passed" with 0 failures.

### 10. Milestone DOD cross-check

1. Run `cd viewer && npx playwright test e2e/renderer-quality.spec.ts` — 7 tests pass (2D renderer)
2. Run `cd viewer && npx playwright test e2e/three-d-view.spec.ts` — 5 tests pass (3D view)
3. Run `cd viewer && npx playwright test e2e/routing-ux.spec.ts` — 5 tests pass (routing flow)
4. Run `cd viewer && npx playwright test e2e/ui-architecture.spec.ts` — all pass (toolbar, view menu, prefs, persistence)
5. Run `cd viewer && npx playwright test e2e/project-manager.spec.ts` — 12 tests pass (project manager)
6. Run `cd viewer && npx playwright test e2e/jlcpcb-search.spec.ts` — 5 tests pass (JLCPCB search + 3D model)
7. **Expected:** All test files pass. This covers DOD items 1-7 and 9.

## Edge Cases

### JLCPCB search error state in browser

1. Start dev server: `cd viewer && npm run dev`
2. Open browser, open search panel via toolbar
3. Open DevTools, go to Network tab, set "Offline" mode
4. Type "10k resistor" in search box
5. **Expected:** Status element shows "Search failed — check connection" with `.error` CSS class. `window.__jlcpcbSearch.lastError` returns an error string.

### Theme cycle through all states

1. Open app in browser
2. Open Preferences modal
3. Click theme button 3 times
4. **Expected:** Button label cycles through light → dark → auto → light. Each click responds immediately (no double-click needed).

## Failure Signals

- Any quality gate stage showing `✗` or non-zero exit code
- Version string mismatch (not `0.1.0-beta` in any of the 3 files)
- Playwright test count below 94 (regression)
- Vitest test count below 127 (regression)
- `#jlcpcb-search-status` showing "No results" on HTTP error instead of error-specific message
- Theme button in Preferences requiring double-click to cycle

## Requirements Proved By This UAT

- No new requirements validated by S07 — this slice verified and polished work from S01-S06

## Not Proven By This UAT

- JLCPCB 3D model loading from production (CORS-limited from localhost, tested via route interception only)
- Desktop-specific behavior (Tauri native menus, file dialogs) — not testable in Playwright web context
- Real-browser performance (headless Chromium performance differs from real GPU)

## Notes for Tester

- The quality gate is the definitive verification — if `bash scripts/quality-gate.sh` passes, the slice is good.
- errors.spec.ts has a historical flake at line 102 ("Ready" vs "Reloaded" race). If it fails once, run again — consistent failure indicates a real regression.
- The theme E2E test asserts on button label, not `data-theme` attribute. This is intentional — `auto` resolves to `light` in headless Chromium.
