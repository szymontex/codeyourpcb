# S07: Polish, Bugs & Verification

**Goal:** All 8 quality gate stages pass, version strings updated to beta, M002 bug fixes verified by E2E, milestone DOD satisfied.
**Demo:** `bash scripts/quality-gate.sh` runs green. Version shows `0.1.0-beta`. Full E2E suite (93+ tests) passes including new prefs-theme single-click test.

## Must-Haves

- ESLint stage passes (unused `showSearchPanel` import removed)
- jscpd stage passes (wasm.ts duplicate eliminated via shared helper)
- `searchComponents()` throws on HTTP errors; panel shows distinct error state vs empty results
- Version strings updated to `0.1.0-beta` in `viewer/package.json`, `Cargo.toml`, `src-tauri/tauri.conf.json`
- E2E test for prefs-theme button single-click (M002 bug fix verification)
- All 8 quality gate stages green
- All existing E2E tests still pass (zero regressions)

## Verification

- `bash scripts/quality-gate.sh` — all 8 stages pass
- `cd viewer && npx playwright test` — full suite passes (93+ tests, including new prefs-theme test)
- `cd viewer && npx vitest run` — all unit tests pass
- `grep '"version"' viewer/package.json src-tauri/tauri.conf.json` shows `0.1.0-beta`
- `grep 'version' Cargo.toml | head -1` shows `0.1.0-beta`
- `cd viewer && npx vitest run -- --grep "JLCPCB"` — JLCPCB unit tests pass including HTTP error throw behavior

## Tasks

- [x] **T01: Fix quality gate failures and improve JLCPCB error handling** `est:45m`
  - Why: Quality gate fails at stages 4 (ESLint) and 8 (jscpd). JLCPCB error state is indistinguishable from empty results (S06 follow-up).
  - Files: `viewer/src/wasm.ts`, `viewer/src/main.ts`, `viewer/src/jlcpcb.ts`, `viewer/src/jlcpcb-panel.ts`
  - Do: (1) Remove `showSearchPanel` from the import in main.ts (keep hideSearchPanel, toggleSearchPanel, isSearchPanelVisible). (2) Extract `buildTraceSegments(segments, layer)` helper in wasm.ts that builds `TraceSegmentInfo[]` + normalizes layer — call from both `WasmPcbEngineAdapter.add_trace` and `MockPcbEngine.add_trace`. (3) Make `searchComponents()` in jlcpcb.ts throw on HTTP errors (non-ok response) — catch only network errors to return `[]`. (4) Update `jlcpcb-panel.ts` to catch search errors and show error CSS class with message instead of "No results". Run `npx eslint src/` and `npx jscpd` after changes.
  - Verify: `cd viewer && npx eslint src/` exits 0. `cd viewer && npx jscpd --exitCode 1` exits 0. `npx vitest run` all pass. `npx playwright test` all pass.
  - Done when: ESLint and jscpd stages pass, JLCPCB error vs empty is distinguishable in UI, zero test regressions.

- [x] **T02: Version naming, M002 bug verification, and milestone DOD signoff** `est:30m`
  - Why: Version strings need beta label. Prefs-theme button lacks E2E coverage (M002 bug). Full quality gate + DOD must be verified.
  - Files: `viewer/package.json`, `Cargo.toml`, `src-tauri/tauri.conf.json`, `viewer/e2e/theme.spec.ts`
  - Do: (1) Update version to `0.1.0-beta` in all 3 files. (2) Add E2E test in theme.spec.ts: open Preferences modal → click `#prefs-theme-btn` → assert `data-theme` changes on `<html>` — verifies M002 single-click bug fix. (3) Run errors.spec.ts 5x to confirm stability. (4) Run full `bash scripts/quality-gate.sh` and cross-check each DOD item from the roadmap.
  - Verify: `bash scripts/quality-gate.sh` passes all 8 stages. `npx playwright test e2e/theme.spec.ts` passes including new test. Version grep shows `0.1.0-beta` in all 3 files.
  - Done when: Quality gate green, prefs-theme E2E passes, errors.spec.ts stable, all 10 DOD items verified.

## Observability / Diagnostics

- `window.__jlcpcbSearch.lastError` — exposes the last search error message (null when no error). Agents and E2E tests can inspect this to verify error state vs empty results.
- Console logs prefixed `[JLCPCB]` — structured messages for search results, HTTP errors, and network failures. `grep` for `[JLCPCB] Search error:` to see failures.
- `JLCPCBSearchError` class — thrown on HTTP errors (4xx/5xx). Type-checkable to distinguish from generic errors.
- Status element `#jlcpcb-search-status` — gains `.error` CSS class on failure. Visible text shows error message vs "No results found" for empty.
- Redaction: no API keys or auth tokens involved (jlcsearch is public, no auth). No secrets to redact.

## Files Likely Touched

- `viewer/src/wasm.ts`
- `viewer/src/main.ts`
- `viewer/src/jlcpcb.ts`
- `viewer/src/jlcpcb-panel.ts`
- `viewer/e2e/theme.spec.ts`
- `viewer/package.json`
- `Cargo.toml`
- `src-tauri/tauri.conf.json`
