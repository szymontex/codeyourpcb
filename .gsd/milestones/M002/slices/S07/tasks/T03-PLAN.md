---
estimated_steps: 5
estimated_files: 8
---

# T03: Playwright E2E tests covering core user flows

**Slice:** S07 — E2E Test Suite & Quality Gates
**Milestone:** M002

## Description

Core deliverable of S07 — automated browser tests with screenshot capture for the most important user actions. This is an EDA app with canvas-based rendering, WASM loading, lazy-loaded 3D, and Monaco editor — each needs specific test patterns. Playwright chosen for screenshot capture, click simulation, and cross-browser capability per roadmap requirements.

Canvas interactions can't use DOM selectors — they need coordinate-based clicks. WASM loading is async with a status indicator. Three.js loads dynamically on first toggle. Tests must account for all of this.

## Steps

1. Install `@playwright/test` as devDependency. Install Chromium browser: `npx playwright install chromium`. Create `viewer/playwright.config.ts` with: baseURL `http://localhost:4321`, webServer command `npm run dev` on port 4321, headless Chromium only, screenshot on failure (`use: { screenshot: 'only-on-failure' }`), `testDir: './e2e'`, 30s timeout. Add `"e2e": "playwright test"` script to package.json.
2. Write `viewer/e2e/app-load.spec.ts` — test WASM initialization: navigate to `/`, wait for status text containing "Ready", verify status bar visible, verify PCB canvas element exists. Verify page title. Take explicit screenshot of initial state for baseline.
3. Write `viewer/e2e/editor.spec.ts` — test editor toggle: click editor toggle button (or Ctrl+E), verify editor panel appears, type `.cypcb` code into Monaco (use `page.keyboard` for Monaco input since it's not a regular input), verify editor panel hides on re-toggle. Write `viewer/e2e/board-interaction.spec.ts` — test layer visibility: toggle Top/Bottom layer checkboxes, verify state persists. Test fit-to-board: press 'F' key, verify no error.
4. Write `viewer/e2e/three-d-view.spec.ts` — test 3D toggle: click 3D button (or press '3'), wait for Three.js canvas to appear, verify `window.__renderer3d.isActive === true` via `page.evaluate`, toggle back to 2D, verify 3D canvas removed. Write `viewer/e2e/undo-redo.spec.ts` — test undo/redo keyboard shortcuts: make a change, Ctrl+Z to undo, Ctrl+Shift+Z to redo. Write `viewer/e2e/theme.spec.ts` — test theme toggle: verify initial theme class, toggle via Ctrl+Shift+T, verify class changes, verify localStorage persistence.
5. Write `viewer/e2e/errors.spec.ts` — test error display: navigate with malformed code that triggers parse errors, verify error panel shows, verify error count badge. Test WASM concepts: verify app reaches ready state. Run full suite: `npx playwright test` — verify ≥15 test cases pass.

## Must-Haves

- [ ] Playwright configured with webServer auto-start on port 4321
- [ ] Screenshot-on-failure enabled
- [ ] App load + WASM ready test passes
- [ ] Editor toggle test passes
- [ ] Layer visibility toggle test passes
- [ ] 3D view toggle test passes (verify renderer active via page.evaluate)
- [ ] Undo/redo keyboard shortcut test passes
- [ ] Theme toggle test passes
- [ ] Error display test passes (malformed input → error shown)
- [ ] ≥15 total E2E test cases across spec files

## Verification

- `cd viewer && npx playwright test` — all pass
- `ls viewer/test-results/` — screenshot artifacts present (from any intentionally-failed test or explicit capture)
- Test output shows ≥15 test cases

## Inputs

- T02 completed — Vitest + ESLint infrastructure exists, package.json has test scripts
- `viewer/src/main.ts` — all UI event listeners, keyboard shortcuts, WASM loading, WebSocket connection
- `viewer/src/renderer3d.ts` — Three.js renderer with `window.__renderer3d` debug surface
- `examples/*.cypcb` — test fixtures including `invalid.cypcb`, `unknown_keyword.cypcb`
- Research: WASM readiness check via `#status-text:has-text("Ready")`, canvas interactions need coordinate-based clicks, Monaco needs keyboard-based input, Three.js lazy-loaded on first click

## Expected Output

- `viewer/playwright.config.ts` — Playwright config with webServer, headless Chromium
- `viewer/e2e/app-load.spec.ts` — WASM init + page load tests
- `viewer/e2e/editor.spec.ts` — editor toggle + code input tests
- `viewer/e2e/board-interaction.spec.ts` — layer toggle + fit-to-board tests
- `viewer/e2e/three-d-view.spec.ts` — 3D toggle + renderer verification
- `viewer/e2e/undo-redo.spec.ts` — undo/redo keyboard shortcut tests
- `viewer/e2e/theme.spec.ts` — theme toggle + persistence tests
- `viewer/e2e/errors.spec.ts` — error display + malformed input tests
