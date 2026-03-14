# S07: E2E Test Suite & Quality Gates — UAT

**Milestone:** M002
**Written:** 2026-03-13

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: All verification is automated — Rust lints, unit tests, E2E browser tests, and quality gate script produce machine-verifiable pass/fail results. No human judgment required.

## Preconditions

- Rust toolchain installed (rustfmt, clippy)
- Node.js installed with npm
- `cd /workspace/codeyourpcb/viewer && npm install` completed
- Playwright browsers installed (`npx playwright install chromium`)
- WASM build available (Vite dev server can start)

## Smoke Test

Run `./scripts/quality-gate.sh` from repo root — should exit 0 with all 6 stages showing ✓.

## Test Cases

### 1. Rust formatting is clean

1. `cd /workspace/codeyourpcb && cargo fmt --check`
2. **Expected:** Exit 0, zero diffs

### 2. Rust lints are clean

1. `cd /workspace/codeyourpcb && cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings`
2. **Expected:** Exit 0, zero warnings

### 3. All Rust tests pass

1. `cd /workspace/codeyourpcb && cargo test --workspace --exclude cypcb-cli --exclude cypcb-desktop`
2. **Expected:** All tests pass, zero failures

### 4. ESLint passes on viewer TypeScript

1. `cd /workspace/codeyourpcb/viewer && npx eslint src/`
2. **Expected:** Exit 0, zero errors

### 5. Vitest unit tests pass

1. `cd /workspace/codeyourpcb/viewer && npx vitest run`
2. **Expected:** 40 tests pass across 4 test files

### 6. Playwright E2E tests pass

1. `cd /workspace/codeyourpcb/viewer && npx playwright test`
2. **Expected:** 39 tests pass across 8 spec files, screenshot artifacts in `test-results/`

### 7. Quality gate script runs end-to-end

1. `cd /workspace/codeyourpcb && ./scripts/quality-gate.sh`
2. **Expected:** All 6 stages pass with ✓ labels, exit 0

### 8. XSS vulnerability is fixed

1. `grep -n 'innerHTML' viewer/src/main.ts`
2. **Expected:** Zero results — no innerHTML usage in main.ts

### 9. Malformed .cypcb files don't crash the app

1. Run Playwright reliability tests: `cd viewer && npx playwright test e2e/reliability.spec.ts`
2. **Expected:** All 7 tests pass — malformed input handled gracefully, canvas remains visible

## Edge Cases

### Empty undo stack keyboard shortcuts

1. With empty undo stack, press Ctrl+Z and Ctrl+Shift+Z
2. **Expected:** No crash, no error — buttons remain disabled

### XSS payload in board content

1. Enter `<img src=x onerror=alert(1)>` as board content via editor
2. **Expected:** Text rendered as literal string in error panel, no script execution

### URL state with extreme values

1. Load page with `?x=999999&y=-999999&z=0.001&layers=F.Cu`
2. **Expected:** App loads without crash, values applied or clamped gracefully

## Failure Signals

- `./scripts/quality-gate.sh` exits non-zero — specific stage label identifies which check failed
- Playwright test failures produce screenshot artifacts in `viewer/test-results/`
- Any `innerHTML` usage in `viewer/src/main.ts` indicates XSS regression

## Requirements Proved By This UAT

- None directly — this slice proves operational quality (test infrastructure) not user-facing features

## Not Proven By This UAT

- Cross-browser compatibility (Firefox, Safari, Edge) — Chromium only
- Code duplication threshold enforcement — no tool defined
- Desktop crate quality (excluded from gates)
- Performance benchmarks (deferred to S08)

## Notes for Tester

- The quality gate script is the single command that proves everything: `./scripts/quality-gate.sh`
- Playwright needs ~17s to run all 39 tests — it auto-starts Vite dev server
- Screenshot artifacts only appear on test failure — `baseline-initial-state.png` is the only persistent screenshot
- Desktop crates are intentionally excluded — they need system libraries not present in dev containers
