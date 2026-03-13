---
id: T02
parent: S05
milestone: M003
provides:
  - 14 E2E tests covering project manager lifecycle, templates, recent files, dismiss/re-show, and editor→board reflow
  - PM-overlay-aware test setup pattern for all existing E2E tests that interact with canvas/editor
  - __editor debug surface for E2E editor content manipulation
key_files:
  - viewer/e2e/project-manager.spec.ts
  - viewer/e2e/board-interaction.spec.ts
  - viewer/e2e/editor.spec.ts
  - viewer/e2e/errors.spec.ts
  - viewer/e2e/reliability.spec.ts
  - viewer/e2e/undo-redo.spec.ts
  - viewer/e2e/ui-architecture.spec.ts
  - viewer/e2e/three-d-view.spec.ts
  - viewer/src/project-manager.ts
  - viewer/src/main.ts
key_decisions:
  - PM dismissal pattern in E2E tests: call __loadBoard(MINIMAL_BOARD) in beforeEach for tests needing canvas/editor access
  - Exposed show/hide on __projectManager debug surface for E2E lifecycle testing (avoids desktop-only event dependency)
  - Exposed __editor on window after Monaco init for E2E editor content manipulation (window.monaco not available in Vite builds)
patterns_established:
  - All E2E tests that interact with canvas or editor must dismiss PM overlay via __loadBoard in beforeEach
  - Editor content can be set via window.__editor.setValue() in E2E tests — triggers onDidChangeModelContent for sync testing
observability_surfaces:
  - "window.__projectManager.show() / .hide() — E2E can trigger PM lifecycle transitions"
  - "window.__editor — direct access to Monaco editor instance after initialization"
  - "window.__projectManager.visible — boolean state check for PM overlay"
duration: 1 session
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: E2E tests and editor→board reflow verification

**Wrote 14 E2E tests proving project manager works end-to-end, verified editor→board reflow, and fixed all 27 PM-overlay regressions in existing E2E suite — full suite 87/87 green.**

## What Happened

1. **Fixed pre-flight observability gaps**: Added diagnostic verification step to S05-PLAN.md and Observability Impact section to T02-PLAN.md.

2. **Expanded `project-manager.spec.ts`** from 9 tests (T01) to 14 tests covering:
   - PM visible on fresh load, toolbar visible while PM shown
   - Template cards present, template click loads board + hides PM
   - Blank board loads 50×50mm scaffold
   - Recent files updated after template load, capped at 10, persist across reload
   - `__loadBoard()` hides PM
   - `showProjectManager()` re-shows PM after dismiss
   - **Editor→board reflow**: load 50mm board → open editor → setValue to 80×60mm → wait 600ms → assert snapshot shows 80_000_000 × 60_000_000 nm

3. **Fixed PM-overlay regressions** in 7 existing test files by adding `__loadBoard(MINIMAL_BOARD)` to `beforeEach` blocks: `board-interaction`, `editor`, `errors`, `reliability`, `undo-redo`, `ui-architecture` (3 describe blocks + 3 persistence tests), and one test in `three-d-view`.

4. **Added debug surfaces**: `show()`/`hide()` on `window.__projectManager`, `window.__editor` on Monaco init.

## Verification

- `npx playwright test e2e/project-manager.spec.ts` — **14/14 pass** ✅
- `npx playwright test` — **87/87 pass** (73 existing + 14 new) ✅
- `npx vitest run` — **109/109 unit tests pass** ✅
- Diagnostic check: `window.__projectManager` returns `{ visible, recentFiles, templateCount, show, hide }` ✅

## Diagnostics

- `window.__projectManager` — `{ visible, recentFiles, templateCount, show(), hide() }` for PM lifecycle inspection
- `window.__editor` — Monaco editor instance (available after first editor toggle); use `.setValue()` to set content, `.getValue()` to read
- `window.__pcbEngine.get_snapshot().board.width_nm` — verify board dimensions after editor→board reflow

## Deviations

- **`desktop:new-file` event not available on web**: The event listener is inside `if (isDesktop())`, so dispatching it in E2E has no effect. Added `show()`/`hide()` to the `__projectManager` debug surface instead. Test verifies the same lifecycle via `__projectManager.show()`.
- **`window.monaco` not available in Vite builds**: Monaco is imported as an ES module, not exposed globally. Added `window.__editor` debug surface in main.ts to enable `editor.setValue()` calls from E2E tests.

## Known Issues

- None. All 87 E2E tests and 109 unit tests pass.

## Files Created/Modified

- `viewer/e2e/project-manager.spec.ts` — REWRITTEN: 14 E2E tests (PM lifecycle, templates, recent files, dismiss/re-show, editor→board reflow)
- `viewer/src/project-manager.ts` — MODIFIED: added `show`/`hide` methods to `__projectManager` debug surface
- `viewer/src/main.ts` — MODIFIED: exposed `window.__editor` after Monaco initialization
- `viewer/e2e/board-interaction.spec.ts` — MODIFIED: added PM dismissal in beforeEach
- `viewer/e2e/editor.spec.ts` — MODIFIED: added PM dismissal in beforeEach
- `viewer/e2e/errors.spec.ts` — MODIFIED: added PM dismissal in beforeEach
- `viewer/e2e/reliability.spec.ts` — MODIFIED: added PM dismissal in beforeEach
- `viewer/e2e/undo-redo.spec.ts` — MODIFIED: added PM dismissal in beforeEach
- `viewer/e2e/ui-architecture.spec.ts` — MODIFIED: added PM dismissal in all 4 describe blocks + persistence tests
- `viewer/e2e/three-d-view.spec.ts` — MODIFIED: added PM dismissal in one test
- `.gsd/milestones/M003/slices/S05/S05-PLAN.md` — MODIFIED: added diagnostic verification step
- `.gsd/milestones/M003/slices/S05/tasks/T02-PLAN.md` — MODIFIED: added Observability Impact section
