---
estimated_steps: 5
estimated_files: 5
---

# T01: Fix quality gate failures and improve JLCPCB error handling

**Slice:** S07 — Polish, Bugs & Verification
**Milestone:** M003

## Description

Two quality gate stages fail: ESLint (unused import) and jscpd (11-line duplicate in wasm.ts). Both are straightforward fixes. Additionally, S06 left searchComponents() swallowing all errors — HTTP failures and empty results are indistinguishable at the UI level. This task fixes all three issues.

## Steps

1. Remove `showSearchPanel` from the import destructuring in `viewer/src/main.ts` line 26. Keep `initSearchPanel`, `hideSearchPanel`, `toggleSearchPanel`, `isSearchPanelVisible`. Run `npx eslint src/` to confirm zero errors.

2. In `viewer/src/wasm.ts`, extract a shared helper function `buildTraceSegments(segments: number[], layer: string): { traceSegments: TraceSegmentInfo[], normalizedLayer: string }` that:
   - Builds `TraceSegmentInfo[]` from the flat `segments` array (4 values per segment)
   - Normalizes layer name (`TopCopper` → `Top`, `BottomCopper` → `Bottom`)
   - Returns both results
   
   Replace the duplicate code in `WasmPcbEngineAdapter.add_trace` (lines ~630-638) and `MockPcbEngine.add_trace` (lines ~762-774) with calls to this helper. Run `npx jscpd --exitCode 1` to confirm zero clones.

3. In `viewer/src/jlcpcb.ts`, modify `searchComponents()` to throw on HTTP errors (non-ok response.status). Keep the try/catch that returns `[]` only for network-level errors (fetch throws). Add a distinct error class or message prefix so the panel can distinguish.

4. In `viewer/src/jlcpcb-panel.ts`, update the search handler to catch errors from `searchComponents()` and display an error state (use existing error CSS class path that was previously unreachable). Show "Search failed — check connection" or similar instead of "No results found".

5. Run full test suites: `npx vitest run` (unit tests) and `npx playwright test` (E2E) to confirm zero regressions. The JLCPCB E2E tests use route interception so the new throw behavior won't affect them.

## Must-Haves

- [ ] ESLint reports zero errors on `viewer/src/`
- [ ] jscpd reports zero clones above threshold in `viewer/src/`
- [ ] `searchComponents()` throws on HTTP errors (4xx/5xx)
- [ ] Search panel shows distinct error message vs "No results"
- [ ] All existing unit and E2E tests pass

## Verification

- `cd viewer && npx eslint src/` — exit code 0
- `cd viewer && npx jscpd --exitCode 1` — exit code 0
- `cd viewer && npx vitest run` — all pass (127+)
- `cd viewer && npx playwright test` — all pass (93)

## Observability Impact

- **`window.__jlcpcbSearch.lastError`** — now populated on HTTP errors (was always null because `searchComponents()` never threw). Agents can read this to distinguish error vs empty.
- **`#jlcpcb-search-status.error`** — CSS class applied on HTTP failures. Visual + DOM-inspectable signal.
- **Console `[JLCPCB] Search error: HTTP {status}`** — logged on non-ok responses before throwing. Survives in browser devtools and E2E captures.
- **`JLCPCBSearchError`** — exported error class. Downstream callers can `instanceof` check to distinguish HTTP errors from other failures.

## Inputs

- `viewer/src/wasm.ts` — contains duplicate trace-building logic at lines ~630-638 and ~762-774
- `viewer/src/main.ts:26` — contains unused `showSearchPanel` import
- `viewer/src/jlcpcb.ts` — `searchComponents()` currently catches all errors and returns `[]`
- `viewer/src/jlcpcb-panel.ts` — has error CSS class path that is unreachable
- S06 summary — documents the error state improvement as a follow-up item

## Expected Output

- `viewer/src/wasm.ts` — shared `buildTraceSegments()` helper, duplicate eliminated
- `viewer/src/main.ts` — clean import (no unused `showSearchPanel`)
- `viewer/src/jlcpcb.ts` — `searchComponents()` throws on HTTP errors
- `viewer/src/jlcpcb-panel.ts` — error state rendered distinctly from empty results
