---
id: S07
parent: M003
milestone: M003
provides:
  - All 8 quality gate stages pass (cargo fmt, clippy, cargo test, eslint, vitest, playwright, autorouter benchmark, jscpd)
  - Version strings updated to 0.1.0-beta across package.json, Cargo.toml, tauri.conf.json
  - ESLint clean — unused showSearchPanel import removed
  - jscpd clean — wasm.ts duplication eliminated via shared buildTraceSegments() helper
  - JLCPCB searchComponents() throws JLCPCBSearchError on HTTP errors; panel shows distinct error vs empty state
  - E2E test for prefs-theme single-click button (M002 bug verification)
  - 94 Playwright E2E tests, 127 Vitest unit tests, 808 Rust tests — all passing
  - All 10 milestone DOD items verified
requires:
  - slice: S03
    provides: Routing UX with testable state machine for E2E
  - slice: S04
    provides: UI architecture, preferences modal, settings persistence
  - slice: S05
    provides: Project manager with recent files and templates
  - slice: S06
    provides: JLCPCB search integration with component results
affects: []
key_files:
  - viewer/src/main.ts
  - viewer/src/wasm.ts
  - viewer/src/jlcpcb.ts
  - viewer/src/jlcpcb-panel.ts
  - viewer/e2e/theme.spec.ts
  - viewer/package.json
  - Cargo.toml
  - src-tauri/tauri.conf.json
key_decisions:
  - JLCPCBSearchError class exported for instanceof-check — network failures return [], HTTP errors throw
  - Prefs-theme E2E asserts button label change, not data-theme — auto resolves to light in headless Chromium
patterns_established:
  - buildTraceSegments() shared helper for trace segment + layer normalization in wasm.ts
observability_surfaces:
  - "window.__jlcpcbSearch.lastError — null on success, string on HTTP error"
  - "#jlcpcb-search-status.error CSS class — applied on search failure, hidden on results"
  - "Console [JLCPCB] prefix — structured search lifecycle messages"
drill_down_paths:
  - .gsd/milestones/M003/slices/S07/tasks/T01-SUMMARY.md
  - .gsd/milestones/M003/slices/S07/tasks/T02-SUMMARY.md
duration: 30m
verification_result: passed
completed_at: 2026-03-13
---

# S07: Polish, Bugs & Verification

**Quality gate clean (8/8 stages), version 0.1.0-beta set, JLCPCB error handling improved, M002 bug fix verified by E2E — milestone M003 DOD fully satisfied with 94 E2E + 127 unit + 808 Rust tests passing.**

## What Happened

Two tasks, both surgical.

**T01** fixed the two failing quality gate stages. ESLint failed because `showSearchPanel` was imported but unused in main.ts — removed it. jscpd flagged duplicate trace-building code in wasm.ts between `WasmPcbEngineAdapter.add_trace` and `MockPcbEngine.add_trace` — extracted a shared `buildTraceSegments(segments, layer)` helper that both call. Also improved JLCPCB error handling: `searchComponents()` now throws `JLCPCBSearchError` on HTTP errors (4xx/5xx) while still returning `[]` for network failures. The search panel catches the error and shows "Search failed — server returned {status}" vs "No results found" for empty searches.

**T02** updated version strings to `0.1.0-beta` in all three locations, added an E2E test verifying the M002 prefs-theme single-click bug stays fixed, confirmed errors.spec.ts stability (5/5 consecutive runs), and ran the full quality gate as final DOD verification. The theme test needed a small iteration — initially asserted `data-theme` change, but the light→dark→auto→light cycle means `auto` resolves to `light` in headless Chromium, so the attribute doesn't always change. Switched to button label assertion which always changes on click.

## Verification

Full quality gate (`bash scripts/quality-gate.sh`) — all 8 stages pass:
- cargo fmt + clippy: clean
- cargo test: 808 passed
- eslint: 0 errors
- vitest: 127 passed (11 suites)
- playwright: 94 passed
- autorouter benchmark: 500-component board in <30s
- jscpd: 0 clones, 0% duplication

Version check: `grep -r '0.1.0-beta'` matches in viewer/package.json, Cargo.toml, src-tauri/tauri.conf.json.

errors.spec.ts: 5/5 consecutive runs pass.

Milestone DOD items (10/10):
1. ✅ 2D board view — renderer-quality.spec.ts (7 tests)
2. ✅ 3D rendering — three-d-view.spec.ts (5 tests)
3. ✅ Routing flow — routing-ux.spec.ts (5 tests)
4. ✅ Preferences + units — ui-architecture.spec.ts prefs tests
5. ✅ Project manager — project-manager.spec.ts (12 tests)
6. ✅ JLCPCB search — jlcpcb-search.spec.ts (5 tests)
7. ✅ Toolbar clean / View menu — ui-architecture.spec.ts structure tests
8. ✅ M002 bugs fixed — theme.spec.ts prefs-theme test
9. ✅ E2E coverage — 94 tests (exceeds 93+ requirement)
10. ✅ Quality gate — 8/8 stages pass

## Deviations

None.

## Known Limitations

- JLCPCB 3D models CORS-limited from localhost — pipeline built and tested via route interception but live fetches blocked in dev
- errors.spec.ts historically flaky at line 102 ("Ready" vs "Reloaded" race) — stable in current runs but pattern noted

## Follow-ups

None — this is the final slice of M003.

## Files Created/Modified

- `viewer/src/main.ts` — removed unused `showSearchPanel` import
- `viewer/src/wasm.ts` — added `buildTraceSegments()` shared helper, replaced duplicate code
- `viewer/src/jlcpcb.ts` — added `JLCPCBSearchError` class, throw on HTTP errors
- `viewer/src/jlcpcb-panel.ts` — distinct error messages for HTTP vs connection failures
- `viewer/e2e/theme.spec.ts` — added prefs-theme single-click E2E test
- `viewer/package.json` — version `0.1.0-beta`
- `Cargo.toml` — version `0.1.0-beta`
- `src-tauri/tauri.conf.json` — version `0.1.0-beta`

## Forward Intelligence

### What the next milestone should know
- Quality gate is solid — 8 stages, 94 E2E + 127 unit + 808 Rust tests. Any new feature should extend existing test patterns.
- JLCPCB search uses tscircuit/jlcsearch (no auth, CORS-enabled) — works in deployed builds but CORS blocks 3D model loading from localhost.
- EasyEDA OBJ parser is custom (~100 lines) due to non-standard format. Three.js OBJLoader can't parse it.

### What's fragile
- errors.spec.ts line 102 — "Ready" vs "Reloaded" status text race condition. Currently stable but the test depends on a timing window.
- Theme cycle in headless Chromium — `auto` resolves to `light`, making `data-theme` attribute tests unreliable. Use button label for assertions.

### Authoritative diagnostics
- `bash scripts/quality-gate.sh` — single command, definitive 8-stage verification
- `window.__renderDiag` — LOD tier, pad-net map size, frame time, text count
- `window.__renderer3d` — 3D geometry counts (components, traces, pads, vias)
- `window.__jlcpcbSearch.lastError` — JLCPCB search error state

### What assumptions changed
- None — S07 was a clean-up slice with no surprises.
