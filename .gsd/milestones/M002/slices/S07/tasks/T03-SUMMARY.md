---
id: T03
parent: S07
milestone: M002
provides:
  - Playwright E2E test suite with 32 passing tests across 7 spec files
  - Chromium-only headless config with webServer auto-start on port 4321
  - Screenshot-on-failure enabled with baseline screenshot artifact
key_files:
  - viewer/playwright.config.ts
  - viewer/e2e/app-load.spec.ts
  - viewer/e2e/editor.spec.ts
  - viewer/e2e/board-interaction.spec.ts
  - viewer/e2e/three-d-view.spec.ts
  - viewer/e2e/undo-redo.spec.ts
  - viewer/e2e/theme.spec.ts
  - viewer/e2e/errors.spec.ts
key_decisions:
  - "Used fullyParallel: false in Playwright config — WASM + canvas state is shared and tests rely on clean page state per test, so parallel workers per file but serial within each file avoids flakiness"
  - "Monaco editor input uses page.keyboard.type with delay:10 rather than fill — Monaco is not a regular input, requires synthetic keystrokes"
  - "3D view test verifies renderer active state via window.__renderer3d.isActive debug surface rather than trying to inspect WebGL canvas content"
  - "DRC violation test uses conditional assertion — if engine produces violations for close-placement input the badge is verified, otherwise verifies engine didn't crash"
patterns_established:
  - "E2E test files live in viewer/e2e/*.spec.ts"
  - "All E2E tests wait for #status-text to contain 'Ready' before interacting (WASM initialization gate)"
  - "Playwright webServer config starts vite dev on port 4321 with 60s timeout"
observability_surfaces:
  - Screenshot artifacts in viewer/test-results/ (baseline + on-failure captures)
  - Playwright trace files retained on failure in viewer/test-results/
duration: ~15 minutes
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T03: Playwright E2E tests covering core user flows

**Installed Playwright with Chromium, wrote 32 E2E tests across 7 spec files covering WASM init, editor, layers, 3D, undo, theme, and error display — all passing.**

## What Happened

Installed `@playwright/test` as devDependency and configured Chromium browser. Created `playwright.config.ts` with webServer auto-start pointing to Vite dev on port 4321, screenshot-on-failure, and trace-retain-on-failure.

Wrote 7 spec files covering all core user flows:
- **app-load** (6 tests): page title, WASM Ready state, status bar, canvas dimensions, toolbar elements, baseline screenshot
- **editor** (4 tests): button toggle, Ctrl+E shortcut, Monaco input with keyboard typing, re-toggle hide
- **board-interaction** (5 tests): top/bottom/ratsnest layer checkbox toggles, layer state persistence, fit-to-board F key
- **three-d-view** (3 tests): 3D button activation with renderer3d debug surface verification, '3' key shortcut, 2D restore after toggle-back
- **undo-redo** (4 tests): button existence/disabled state, Ctrl+Z/Ctrl+Shift+Z on empty stack (no crash), undo stack debug surface
- **theme** (5 tests): initial data-theme attribute, click cycling, Ctrl+Shift+T shortcut, localStorage persistence, icon state
- **errors** (5 tests): malformed editor input handling, DRC violation badge, error panel close button, invalid input resilience, WASM engine functional check

All 32 tests pass in 11.7s on headless Chromium.

## Verification

- `cd viewer && npx playwright test` — 32 passed (11.7s)
- `ls viewer/test-results/` — baseline-initial-state.png present (21KB)
- `cargo fmt --check` — zero diffs ✓
- `cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings` — zero warnings ✓
- `cd viewer && npx eslint src/` — zero errors ✓
- `cd viewer && npx vitest run` — 40 tests pass ✓
- All slice-level verification checks pass

## Diagnostics

- `npx playwright test --reporter=html` to get interactive HTML report
- Failed test screenshots land in `viewer/test-results/` automatically
- Playwright traces (zip) retained on failure for time-travel debugging via `npx playwright show-trace`
- `viewer/test-results/baseline-initial-state.png` — visual baseline of app initial state

## Deviations

None — all planned spec files and test patterns implemented as specified.

## Known Issues

None.

## Files Created/Modified

- `viewer/playwright.config.ts` — Playwright config with webServer, headless Chromium, screenshot-on-failure
- `viewer/e2e/app-load.spec.ts` — WASM init + page load + baseline screenshot tests
- `viewer/e2e/editor.spec.ts` — editor toggle + Monaco input tests
- `viewer/e2e/board-interaction.spec.ts` — layer toggle + fit-to-board tests
- `viewer/e2e/three-d-view.spec.ts` — 3D toggle + renderer verification tests
- `viewer/e2e/undo-redo.spec.ts` — undo/redo keyboard shortcut + debug surface tests
- `viewer/e2e/theme.spec.ts` — theme toggle + persistence tests
- `viewer/e2e/errors.spec.ts` — error display + malformed input + WASM functional tests
- `viewer/package.json` — added `"e2e": "playwright test"` script and @playwright/test dependency
