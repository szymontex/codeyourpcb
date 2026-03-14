---
id: T04
parent: S07
milestone: M002
provides:
  - innerHTML XSS vulnerability fixed — error panel uses DOM API with textContent only
  - 7 Playwright reliability E2E tests (malformed input + URL state roundtrip)
  - scripts/quality-gate.sh — single executable running all 6 quality stages
key_files:
  - viewer/src/main.ts
  - viewer/e2e/reliability.spec.ts
  - scripts/quality-gate.sh
key_decisions:
  - "Replaced innerHTML with createElement+textContent for error panel: builds DOM nodes programmatically, click handlers attached inline during construction instead of post-query"
patterns_established:
  - "Quality gate script uses pass/fail functions with ✓/✗ labels and exits on first failure"
  - "Reliability E2E tests use editor input (Monaco keyboard.type) to inject malformed content into WASM engine"
observability_surfaces:
  - "scripts/quality-gate.sh outputs per-stage pass/fail with labels [1/6]–[6/6]"
duration: ~20min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T04: Input sanitization, web reliability tests, and quality gate script

**Fixed innerHTML XSS in error panel, added 7 E2E reliability tests, created quality gate script — all 6 stages pass (39 E2E + 40 unit + Rust lint/test + ESLint).**

## What Happened

1. **XSS fix**: Replaced `errorList.innerHTML` with programmatic DOM construction (`createElement` + `textContent`). The error panel now builds each violation item as DOM nodes with text content, never inserting user-controlled strings as HTML. Click handlers for zoom-to-location are attached during construction rather than via post-query.

2. **Reliability E2E tests** (`viewer/e2e/reliability.spec.ts`): 7 tests across two describe blocks:
   - Malformed input (4 tests): missing values, unknown keywords, garbage input, XSS payload — all verify no crash and canvas remains visible
   - URL state roundtrip (3 tests): params applied on load, share button produces valid URL, roundtrip preserves values

3. **Quality gate script** (`scripts/quality-gate.sh`): Runs 6 stages in order — cargo fmt, clippy, cargo test, eslint, vitest, playwright — with labeled pass/fail output and exit-on-first-failure.

## Verification

- `grep -n 'innerHTML' viewer/src/main.ts` → zero results
- `cd viewer && npx playwright test e2e/reliability.spec.ts` → 7 passed
- `./scripts/quality-gate.sh` → all 6 stages ✓, exit 0
  - 39 Playwright E2E tests passed
  - 40 Vitest unit tests passed
  - Zero clippy warnings, zero fmt diffs, zero ESLint errors

## Diagnostics

- `./scripts/quality-gate.sh` — run from repo root to verify all quality checks
- Playwright failure screenshots in `viewer/test-results/`
- Stage labels `[1/6]`–`[6/6]` identify which check failed

## Deviations

- Test used `#board-canvas` initially — corrected to `#pcb-canvas` matching actual HTML.

## Known Issues

None.

## Files Created/Modified

- `viewer/src/main.ts` — XSS fix: replaced innerHTML with DOM API for error panel
- `viewer/e2e/reliability.spec.ts` — 7 new E2E tests for malformed input and URL state roundtrip
- `scripts/quality-gate.sh` — quality gate script running all 6 check stages
