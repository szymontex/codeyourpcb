---
id: T01
parent: S07
milestone: M003
provides:
  - ESLint and jscpd quality gate stages pass
  - JLCPCB search HTTP errors surfaced distinctly from empty results
  - Shared buildTraceSegments helper eliminates wasm.ts duplication
key_files:
  - viewer/src/main.ts
  - viewer/src/wasm.ts
  - viewer/src/jlcpcb.ts
  - viewer/src/jlcpcb-panel.ts
key_decisions:
  - JLCPCBSearchError class exported so downstream can instanceof-check HTTP errors
  - Network-level failures (fetch throws) still return [] — only server errors throw
patterns_established:
  - buildTraceSegments() shared helper for trace segment + layer normalization
observability_surfaces:
  - window.__jlcpcbSearch.lastError — populated on HTTP errors, null otherwise
  - "#jlcpcb-search-status.error" CSS class — applied on search failure
  - Console "[JLCPCB] Search error: HTTP {status}" — logged before throwing
duration: 15m
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Fix quality gate failures and improve JLCPCB error handling

**Removed unused import, eliminated wasm.ts duplication via shared helper, and made searchComponents() throw on HTTP errors with distinct error display in the search panel.**

## What Happened

1. Removed `showSearchPanel` from the import in `main.ts` — it was imported but never used. `initSearchPanel`, `hideSearchPanel`, `toggleSearchPanel`, `isSearchPanelVisible` remain.

2. Extracted `buildTraceSegments(segments, layer)` helper in `wasm.ts` that builds `TraceSegmentInfo[]` from a flat coordinate array and normalizes the layer name. Replaced identical logic in both `WasmPcbEngineAdapter.add_trace` and `MockPcbEngine.add_trace`.

3. Added `JLCPCBSearchError` class in `jlcpcb.ts`. Modified `searchComponents()` to throw it on non-ok HTTP responses. The outer catch re-throws `JLCPCBSearchError` and only swallows network-level failures (returns `[]`).

4. Updated `jlcpcb-panel.ts` to import `JLCPCBSearchError` and show a specific message: "Search failed — server returned {status}" for HTTP errors, "Search failed — check connection" for other errors. The panel's existing catch block was already wired to `showStatus(msg, true)` — it just needed the error to actually reach it.

## Verification

- `cd viewer && npx eslint src/` — exit code 0, zero errors
- `cd viewer && npx jscpd --exitCode 1` — exit code 0, zero clones
- `cd viewer && npx vitest run` — 127 tests passed (11 suites)
- `cd viewer && npx playwright test` — 93 tests passed, including JLCPCB search tests (empty results, API error via route.abort, debounce, 3D model pipeline)

Slice-level verification (partial — T02 still pending):
- ✅ ESLint stage passes
- ✅ jscpd stage passes
- ✅ vitest all pass (127)
- ✅ playwright all pass (93)
- ⬜ Version strings — still `0.1.0`, T02 will update to `0.1.0-beta`
- ⬜ quality-gate.sh full run — T02
- ⬜ prefs-theme E2E test — T02

## Diagnostics

- `window.__jlcpcbSearch.lastError` — inspect in browser console or E2E to check if the last search triggered an error (null = success or empty, string = error message)
- `#jlcpcb-search-status` element — has `.error` class when showing an error state, hidden when results are displayed
- Console filter `[JLCPCB]` — all search-related log messages use this prefix

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `viewer/src/main.ts` — removed unused `showSearchPanel` import
- `viewer/src/wasm.ts` — added `buildTraceSegments()` helper, replaced duplicate code in both `add_trace` implementations
- `viewer/src/jlcpcb.ts` — added `JLCPCBSearchError` class, `searchComponents()` now throws on HTTP errors
- `viewer/src/jlcpcb-panel.ts` — imported `JLCPCBSearchError`, user-facing error messages distinguish HTTP errors from connection failures
