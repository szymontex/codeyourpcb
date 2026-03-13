# S07: E2E Test Suite & Quality Gates

**Goal:** Every user action covered by automated tests, all linters passing, inputs sanitized, errors user-friendly, web reliability edge cases exercised — with a single `make quality` gate script that gates CI.
**Demo:** Run `make quality` (or equivalent script) from repo root — all Rust tests pass, clippy clean, rustfmt clean, ESLint clean, Vitest unit tests pass, Playwright E2E tests pass with screenshot artifacts.

## Must-Haves

- All Rust lints clean: `cargo fmt --check` zero diffs, `cargo clippy --workspace -- -D warnings` zero warnings (excluding uncompilable desktop crates)
- ESLint configured and passing for all viewer TypeScript
- Vitest unit tests for pure modules: viewport, hit-test, undo, url-state (≥20 test cases)
- Playwright E2E tests covering core user flows: app load/WASM init, editor toggle, code edit→render, 3D toggle, layer visibility, undo/redo, theme toggle, keyboard shortcuts, error display (≥15 test cases with screenshot capture on failure)
- innerHTML XSS in error panel fixed (HTML-escaped)
- Malformed `.cypcb` handling verified (no crash, user-friendly error)
- WASM load failure → mock engine fallback verified in E2E
- Single quality gate script that runs all checks and exits non-zero on any failure

## Proof Level

- This slice proves: operational (full test + lint infrastructure running against real app)
- Real runtime required: yes (Playwright needs dev server + WASM build)
- Human/UAT required: no (all verification is automated)

## Verification

- `cd /workspace/codeyourpcb && cargo fmt --check` — zero diffs
- `cd /workspace/codeyourpcb && cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings` — zero warnings
- `cd /workspace/codeyourpcb/viewer && npx eslint src/` — zero errors
- `cd /workspace/codeyourpcb/viewer && npx vitest run` — all pass
- `cd /workspace/codeyourpcb/viewer && npx playwright test` — all pass, screenshot artifacts in `test-results/`
- `cd /workspace/codeyourpcb && ./scripts/quality-gate.sh` — exits 0

## Observability / Diagnostics

- Runtime signals: Playwright screenshot-on-failure artifacts in `viewer/test-results/`, Vitest coverage report
- Inspection surfaces: `./scripts/quality-gate.sh` outputs pass/fail per stage with exit code
- Failure visibility: each quality gate stage reports its own failure independently (fmt, clippy, eslint, vitest, playwright)

## Integration Closure

- Upstream surfaces consumed: `viewer/src/*.ts` (all UI code), `crates/*/` (all Rust crates), `examples/*.cypcb` (test fixtures)
- New wiring introduced: Playwright config → Vite dev server, Vitest config → existing vite.config.ts, ESLint flat config, quality gate script
- What remains: S08 (performance benchmarks, final polish) — quality gates established here will gate S08 work too

## Tasks

- [x] **T01: Rust lint cleanup — cargo fmt + clippy fix** `est:45m`
  - Why: 680 rustfmt diffs and 122 clippy warnings block quality gates. Must go first to avoid merge conflicts with later work.
  - Files: all `crates/*/src/*.rs` files, `.cargo/config.toml` (if clippy config needed)
  - Do: Run `cargo fmt` across workspace (single commit). Then `cargo clippy --fix --workspace --exclude cypcb-cli --exclude cypcb-desktop --allow-dirty`. Manually fix remaining clippy warnings (ptr_arg in parser — use `#[allow]` with justification, too-many-arguments — restructure or allow). Fix the 3 pre-existing test failures (2 export/filesystem, 1 world/sync).
  - Verify: `cargo fmt --check` zero diffs, `cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings` zero warnings, `cargo test --workspace --exclude cypcb-cli --exclude cypcb-desktop` all pass
  - Done when: Rust codebase is fmt-clean, clippy-clean with `-D warnings`, and all tests pass

- [x] **T02: Frontend lint + Vitest unit tests for pure modules** `est:45m`
  - Why: No frontend test or lint infrastructure exists. Pure modules (viewport, hit-test, undo, url-state) are ideal first test targets — no DOM deps, fast feedback, validates test setup works.
  - Files: `viewer/eslint.config.js`, `viewer/vitest.config.ts`, `viewer/package.json`, `viewer/src/__tests__/viewport.test.ts`, `viewer/src/__tests__/hit-test.test.ts`, `viewer/src/__tests__/undo.test.ts`, `viewer/src/__tests__/url-state.test.ts`
  - Do: Install vitest + eslint + @typescript-eslint as devDependencies. Create ESLint v9 flat config for TypeScript. Create vitest.config.ts extending vite.config.ts. Write unit tests for: viewport (worldToScreen/screenToWorld roundtrip, zoomAtPoint, fitBoard), hit-test (hitTestTrace with various geometries), undo (push/undo/redo/clear/capacity), url-state (encode/decode roundtrip). Fix any ESLint errors surfaced. Add `test` and `lint` scripts to package.json.
  - Verify: `npx eslint src/` zero errors, `npx vitest run` all pass (≥20 test cases)
  - Done when: ESLint configured and passing, Vitest running with ≥20 unit tests across 4 modules

- [x] **T03: Playwright E2E tests covering core user flows** `est:60m`
  - Why: Core deliverable of this slice — automated browser tests with screenshot capture covering the 15+ most important user actions. Canvas-based EDA app needs coordinate-aware click simulation and WASM readiness waits.
  - Files: `viewer/playwright.config.ts`, `viewer/package.json`, `viewer/e2e/app-load.spec.ts`, `viewer/e2e/editor.spec.ts`, `viewer/e2e/board-interaction.spec.ts`, `viewer/e2e/three-d-view.spec.ts`, `viewer/e2e/undo-redo.spec.ts`, `viewer/e2e/theme.spec.ts`, `viewer/e2e/errors.spec.ts`
  - Do: Install @playwright/test. Create playwright.config.ts with webServer pointing to Vite dev (port 4321), headless Chromium, screenshot-on-failure. Write E2E specs: (1) app-load — WASM init to "Ready" status, (2) editor — toggle open/close, type code, verify render updates, (3) board-interaction — zoom, fit-to-board, layer toggles, (4) three-d-view — toggle 3D, verify canvas exists, toggle back, (5) undo-redo — make change, undo, redo, verify state, (6) theme — toggle dark↔light, verify CSS class, (7) errors — load malformed .cypcb, verify error display, WASM failure fallback. Use `page.waitForSelector` for WASM readiness. Canvas interactions use coordinate-based clicks where needed.
  - Verify: `npx playwright test` all pass, screenshot artifacts present in `test-results/` on failure
  - Done when: ≥15 E2E test cases pass across 7 spec files, covering app load, editor, board interaction, 3D toggle, undo/redo, theme, and error handling

- [x] **T04: Input sanitization, web reliability tests, and quality gate script** `est:40m`
  - Why: Closes remaining must-haves — XSS fix, web reliability edge cases, and the single quality gate script that ties everything together for CI.
  - Files: `viewer/src/main.ts` (innerHTML XSS fix), `viewer/e2e/reliability.spec.ts`, `scripts/quality-gate.sh`
  - Do: Fix innerHTML XSS in error panel (escape HTML entities before insertion). Add Playwright reliability tests: malformed .cypcb files (load `examples/invalid.cypcb`, `examples/unknown_keyword.cypcb` — verify error display, no crash), URL state roundtrip (encode view → navigate → verify decoded state). Create `scripts/quality-gate.sh` that runs in sequence: cargo fmt --check, cargo clippy (with -D warnings, excluding desktop crates), cargo test (excluding desktop), eslint, vitest, playwright — exits non-zero on first failure with clear stage labels.
  - Verify: `./scripts/quality-gate.sh` exits 0, innerHTML in error panel uses text escaping not raw insertion
  - Done when: XSS fixed, reliability E2E tests pass, quality gate script runs all checks end-to-end and exits 0

## Files Likely Touched

- All `crates/*/src/*.rs` (fmt + clippy)
- `viewer/eslint.config.js`
- `viewer/vitest.config.ts`
- `viewer/playwright.config.ts`
- `viewer/package.json`
- `viewer/src/__tests__/viewport.test.ts`
- `viewer/src/__tests__/hit-test.test.ts`
- `viewer/src/__tests__/undo.test.ts`
- `viewer/src/__tests__/url-state.test.ts`
- `viewer/e2e/app-load.spec.ts`
- `viewer/e2e/editor.spec.ts`
- `viewer/e2e/board-interaction.spec.ts`
- `viewer/e2e/three-d-view.spec.ts`
- `viewer/e2e/undo-redo.spec.ts`
- `viewer/e2e/theme.spec.ts`
- `viewer/e2e/errors.spec.ts`
- `viewer/e2e/reliability.spec.ts`
- `viewer/src/main.ts` (XSS fix)
- `scripts/quality-gate.sh`
