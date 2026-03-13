---
estimated_steps: 4
estimated_files: 4
---

# T04: Input sanitization, web reliability tests, and quality gate script

**Slice:** S07 — E2E Test Suite & Quality Gates
**Milestone:** M002

## Description

Closes three remaining must-haves: (1) fix the innerHTML XSS vulnerability in the error panel, (2) add Playwright tests for web reliability edge cases (malformed files, URL state roundtrip), (3) create the quality gate script that runs all checks in sequence. The gate script is the capstone — it's what CI runs and what proves the slice is done.

## Steps

1. Fix innerHTML XSS in `viewer/src/main.ts` — the error panel inserts DRC violation text (which comes from user `.cypcb` content) via `innerHTML`. Replace with `textContent` or create elements programmatically with `document.createElement` + `textContent`. Verify the fix handles the existing error display markup (badges, lists) — may need to build DOM nodes instead of HTML strings.
2. Write `viewer/e2e/reliability.spec.ts` — malformed file handling: use `page.evaluate` to inject malformed `.cypcb` source into the engine, verify error panel appears with message, verify no crash/blank screen. URL state roundtrip: navigate with view state params, verify viewport matches, encode current state, verify URL contains expected params.
3. Create `scripts/quality-gate.sh` — executable bash script that runs each stage in sequence with clear labels, exits non-zero on first failure:
   - Stage 1: `cargo fmt --check`
   - Stage 2: `cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings`
   - Stage 3: `cargo test --workspace --exclude cypcb-cli --exclude cypcb-desktop`
   - Stage 4: `cd viewer && npx eslint src/`
   - Stage 5: `cd viewer && npx vitest run`
   - Stage 6: `cd viewer && npx playwright test`
   Each stage prints `✓ stage_name` on success or `✗ stage_name` and exits on failure.
4. Run `./scripts/quality-gate.sh` end-to-end. Fix any issues that surface from running the full pipeline. Verify exit 0.

## Must-Haves

- [ ] innerHTML XSS in error panel fixed — user-controlled text never inserted as raw HTML
- [ ] Malformed `.cypcb` E2E test passes (error displayed, no crash)
- [ ] URL state roundtrip E2E test passes
- [ ] `scripts/quality-gate.sh` exists, is executable, runs all 6 stages
- [ ] `./scripts/quality-gate.sh` exits 0

## Verification

- `grep -n 'innerHTML' viewer/src/main.ts` — either zero results or only safe static HTML (no user-controlled content)
- `cd viewer && npx playwright test e2e/reliability.spec.ts` — pass
- `./scripts/quality-gate.sh` — exits 0 with all stages showing ✓

## Inputs

- T01 completed — Rust lint clean
- T02 completed — ESLint + Vitest passing
- T03 completed — Playwright E2E suite passing
- Research: `errorList.innerHTML` in main.ts inserts DRC violation text without HTML escaping
- `examples/invalid.cypcb`, `examples/unknown_keyword.cypcb` — malformed file fixtures

## Expected Output

- `viewer/src/main.ts` — XSS-safe error panel rendering
- `viewer/e2e/reliability.spec.ts` — malformed file + URL state roundtrip tests
- `scripts/quality-gate.sh` — single executable quality gate
