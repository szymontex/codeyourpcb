# S05: Project Manager & File Handling

**Goal:** App opens to a project manager screen showing recent files and starter templates. User can create new project from template, import existing `.cypcb`, or open recent. Editor changes trigger board view update/reflow.
**Demo:** Launch app → see project manager with 3 templates and recent files list → click "Blink LED" template → editor fills with source, board renders, project manager dismisses → save → relaunch → recent files shows the previously opened file with name and date.

## Must-Haves

- Project manager overlay shown on app startup when no file is loaded
- Recent files list (name, date, optional thumbnail) persisted to localStorage via settings module
- Template gallery with 3 bundled templates (Blink LED, Power Indicator, Simple PSU) + Blank scaffold
- Import button and Open button work from project manager to load `.cypcb` files
- Project manager dismisses on file load (from any source: template, recent, open, drag-drop)
- "New Project" action (web) clears current file and re-shows project manager
- Editor→board reflow verified working for all mutation types
- E2E tests cover project manager visibility, template loading, recent files persistence

## Proof Level

- This slice proves: operational
- Real runtime required: yes
- Human/UAT required: no (E2E covers the flows)

## Verification

- `npx playwright test e2e/project-manager.spec.ts` — all project manager E2E tests pass
- `npx playwright test` — full E2E suite passes (no regressions)
- `npx vitest run` — all unit tests pass
- Diagnostic check: `window.__projectManager` returns `{ visible, recentFiles, templateCount }` — confirms debug surface is live and inspectable for failure triage

## Observability / Diagnostics

- Runtime signals: `window.__projectManager` exposes `{ visible: boolean, recentFiles: RecentFile[], templateCount: number }` for E2E
- Inspection surfaces: `localStorage.getItem('cypcb-settings')` → `recentFiles` array inspectable in devtools
- Failure visibility: console.warn on thumbnail generation failure (non-fatal, shows placeholder)

## Integration Closure

- Upstream surfaces consumed: `viewer/src/settings.ts` (getPreference/setPreference/subscribe), `viewer/src/renderer.ts` (render() for thumbnails), `viewer/src/file-access.ts` (openFile), `viewer/index.html` (overlay patterns from prefs-overlay)
- New wiring introduced: `project-manager.ts` module imported by main.ts, `showProjectManager()`/`hideProjectManager()` API called from file-load flows and new-file events
- What remains before the milestone is truly usable end-to-end: S06 (JLCPCB integration), S07 (polish & verification)

## Tasks

- [x] **T01: Build project manager module, HTML overlay, templates, and recent files** `est:2h`
  - Why: Core slice deliverable — the project manager overlay, template bundling, recent files tracking, and all wiring into main.ts
  - Files: `viewer/src/project-manager.ts`, `viewer/src/settings.ts`, `viewer/index.html`, `viewer/src/main.ts`, `viewer/public/templates/blink.cypcb`, `viewer/public/templates/power-indicator.cypcb`, `viewer/public/templates/simple-psu.cypcb`
  - Do: (1) Add `recentFiles` array to `AppSettings` with `RecentFileEntry` type (name, timestamp, thumbnail data URL). (2) Copy 3 example files to `viewer/public/templates/`. (3) Create `project-manager.ts` with `showProjectManager()`/`hideProjectManager()`/`addRecentFile()`/`generateThumbnail()` API. (4) Add HTML overlay to `index.html` (between toolbar and main-content, z-index between canvas and prefs-overlay). (5) Wire into main.ts: show on startup when no file loaded, hide on any file load, re-show on new-file event, update recent files on open/save. (6) Expose `window.__projectManager` debug surface. (7) Ensure Ctrl+N / New button dispatches new-file event on web.
  - Verify: `npm run build` succeeds, dev server shows project manager on load, clicking template loads board, recent files list updates after opening a file
  - Done when: Project manager appears on fresh load, templates load boards, recent files persist across page reload, dismiss/show lifecycle works

- [x] **T02: E2E tests and editor→board reflow verification** `est:1h`
  - Why: Proves the slice works end-to-end and verifies the editor→board sync deliverable from the roadmap
  - Files: `viewer/e2e/project-manager.spec.ts`, `viewer/e2e/app-load.spec.ts`
  - Do: (1) Write E2E tests covering: project manager visible on fresh load, template click loads board and dismisses overlay, recent files appear after loading a file and reloading page, open button from project manager works, new-file re-shows project manager. (2) Verify editor→board reflow: type board size change in editor → assert board dimensions update in snapshot. (3) Update app-load.spec.ts if project manager overlay affects existing element visibility checks. (4) Run full suite to confirm zero regressions.
  - Verify: `npx playwright test e2e/project-manager.spec.ts` passes, `npx playwright test` full suite passes, `npx vitest run` passes
  - Done when: All new E2E tests pass, full test suite green, editor→board sync verified by test

## Files Likely Touched

- `viewer/src/project-manager.ts` — NEW: project manager module
- `viewer/src/settings.ts` — Add `recentFiles` to AppSettings
- `viewer/index.html` — Project manager overlay HTML + CSS
- `viewer/src/main.ts` — Wire project manager show/hide/events
- `viewer/public/templates/blink.cypcb` — Bundled template
- `viewer/public/templates/power-indicator.cypcb` — Bundled template
- `viewer/public/templates/simple-psu.cypcb` — Bundled template
- `viewer/e2e/project-manager.spec.ts` — NEW: E2E tests
- `viewer/e2e/app-load.spec.ts` — Update for project manager overlay
