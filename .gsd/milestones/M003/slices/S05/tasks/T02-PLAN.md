---
estimated_steps: 4
estimated_files: 3
---

# T02: E2E tests and editor→board reflow verification

**Slice:** S05 — Project Manager & File Handling
**Milestone:** M003

## Description

Write E2E tests proving the project manager works end-to-end: visible on startup, templates load boards, recent files persist, overlay dismisses and re-shows correctly. Also verify the editor→board reflow that the roadmap requires — type a board size change in the editor and assert the board dimensions update in the snapshot. Update any existing E2E tests broken by the project manager overlay.

## Steps

1. **Write `project-manager.spec.ts`** — E2E tests covering:
   - Project manager overlay visible on fresh page load (no file loaded)
   - Toolbar still visible while project manager is shown
   - Template card click (e.g. "Blink LED") loads board, dismisses project manager, editor has content
   - "Blank" template creates empty scaffold board
   - Open button from project manager triggers file picker
   - After loading a template, `window.__projectManager.recentFiles` has 1 entry
   - Page reload shows project manager with recent file listed
   - Clicking a recent file entry loads that board (via template re-fetch or content, depending on implementation)
   - `__loadBoard(source)` hides project manager (tests existing load flow)
   - New-file action re-shows project manager

2. **Verify editor→board reflow** — In the same spec or a dedicated test:
   - Load a board via `__loadBoard()`
   - Modify editor content (change board size from e.g. 50mm to 80mm)
   - Wait for debounce (400ms)
   - Assert board dimensions changed via `window.__renderDiag` or snapshot inspection
   - This verifies the `setupEditorSync()` pipeline works end-to-end

3. **Update `app-load.spec.ts`** — If project manager overlay covers or changes initial element visibility, update existing tests to either dismiss the overlay first (via `__loadBoard()`) or adjust selectors. The project manager should NOT cover the toolbar (it sits below toolbar, above main-content), so most existing tests should be fine. Verify and fix any that fail.

4. **Run full suite and confirm green** — `npx playwright test` for all E2E, `npx vitest run` for unit tests. Fix any regressions introduced by T01 changes.

## Must-Haves

- [ ] `project-manager.spec.ts` with tests for overlay visibility, template loading, recent files, dismiss/re-show
- [ ] Editor→board reflow test asserting board dimension change propagates through debounced sync
- [ ] No regressions in existing E2E suite
- [ ] Full E2E suite passes
- [ ] Full unit test suite passes

## Verification

- `npx playwright test e2e/project-manager.spec.ts` — all new tests pass
- `npx playwright test` — full suite green (73+ existing + new tests)
- `npx vitest run` — all unit tests pass

## Inputs

- `viewer/src/project-manager.ts` — T01's project manager module with `window.__projectManager` debug surface
- `viewer/src/main.ts` — T01's wiring with show/hide on file load events
- `viewer/e2e/app-load.spec.ts` — existing tests that may need overlay-aware updates
- `viewer/e2e/ui-architecture.spec.ts` — reference for E2E patterns (page.evaluate, __settings, __loadBoard)

## Expected Output

- `viewer/e2e/project-manager.spec.ts` — NEW: 8-12 E2E tests covering project manager and editor→board reflow
- `viewer/e2e/app-load.spec.ts` — MODIFIED if needed: overlay-aware test adjustments

## Observability Impact

- **E2E test coverage surface**: New tests exercise `window.__projectManager` debug surface (visible, recentFiles, templateCount) — future agents can inspect PM state via `page.evaluate(() => window.__projectManager)` in any E2E test.
- **Editor→board sync verification**: Tests validate that `setupEditorSync()` pipeline works end-to-end — if the debounced sync breaks, this test fails with a board-dimension mismatch.
- **PM dismissal pattern**: Tests establish the `__loadBoard()` pattern for dismissing the PM overlay in test setup — existing and future E2E tests that need canvas access should call this in `beforeEach`.
- **Failure visibility**: If PM overlay blocks canvas interactions, tests timeout with a clear pattern (30s timeout on canvas click) rather than a subtle logic error.
