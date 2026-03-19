---
id: T01
parent: S01
milestone: M005
provides:
  - WorkerRequest and WorkerResponse discriminated union types in worker-protocol.ts
  - parseSource() exported from parse-source.ts as a shared module
  - wasm.ts imports parseSource from parse-source.ts (no duplicate definition)
key_files:
  - viewer/src/worker-protocol.ts
  - viewer/src/parse-source.ts
  - viewer/src/wasm.ts
key_decisions:
  - Extracted parseUnit() and getFootprintPads() alongside parseSource() since they are private helpers only used by the parser
  - Cleaned up unused type imports from wasm.ts (PadInfo, PinRef, BoardInfo, ComponentInfo, NetInfo) to avoid lint noise
patterns_established:
  - Worker protocol types use TypeScript discriminated unions with `type` field as discriminant — all future worker messages must extend WorkerRequest or WorkerResponse
  - Shared modules between worker and main thread live at viewer/src/ top level (not in a subdirectory)
observability_surfaces:
  - none (pure refactor — no runtime behavior change)
duration: 15m
verification_result: passed
completed_at: 2026-03-18
blocker_discovered: false
---

# T01: Create worker protocol types and extract parseSource to shared module

**Extracted parseSource() and helpers to shared parse-source.ts module; created worker-protocol.ts with WorkerRequest/WorkerResponse discriminated union types for worker↔main messaging**

## What Happened

Created two new shared modules that the Web Worker (T02) and main thread refactor (T03) will depend on:

1. **`viewer/src/worker-protocol.ts`** — Defines `WorkerRequest` (route, route-with-params, route-variants) and `WorkerResponse` (ready, route-result, variant-result, error) as TypeScript discriminated unions. Each variant is also exported as a named interface for consumers that need to match on specific message types.

2. **`viewer/src/parse-source.ts`** — Extracted the `parseSource()` function and its two private helpers (`parseUnit()`, `getFootprintPads()`) from `wasm.ts`. The function signature and behavior are identical — it parses `.cypcb` DSL source text into a `BoardSnapshot`.

3. **Updated `viewer/src/wasm.ts`** — Removed the ~260 lines of parser code (parseUnit, getFootprintPads, parseSource) and added `import { parseSource } from './parse-source'`. Cleaned up unused type imports that were only needed by the extracted parser.

## Verification

- `npx tsc --noEmit` — only pre-existing `showVariants` unused import error (TS6133), zero new errors
- `npx vitest run --reporter=verbose` — all 127 tests pass across 11 test files
- `grep -c "parseSource" viewer/src/wasm.ts` → 3 (1 import + 2 call sites, no function definition)
- `grep "export.*WorkerRequest\|export.*WorkerResponse" viewer/src/worker-protocol.ts` → both exported

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cd viewer && npx tsc --noEmit` | 2 (pre-existing TS6133 only) | ✅ pass (no new errors) | 4.1s |
| 2 | `cd viewer && npx vitest run --reporter=verbose` | 0 | ✅ pass (127/127 tests) | 4.1s |
| 3 | `grep -c "parseSource" viewer/src/wasm.ts` | 0 | ✅ pass (3 — import + 2 calls) | <1s |
| 4 | `grep "export.*WorkerRequest\|export.*WorkerResponse" viewer/src/worker-protocol.ts` | 0 | ✅ pass (both found) | <1s |

### Slice-level verification (partial — T01 is task 1 of 5)

| # | Check | Status |
|---|-------|--------|
| 1 | `npx playwright test viewer/e2e/autoroute-worker.spec.ts` | ⏳ not yet applicable (E2E test created in T05) |
| 2 | Manual dev server verification | ⏳ not yet applicable (worker integration in T03) |
| 3 | Diagnostic failure-path check | ⏳ not yet applicable (worker error handling in T02/T03) |

## Diagnostics

No runtime diagnostics — this is a pure code extraction/refactor. To verify the modules are correct:
- Import `parseSource` from `./parse-source` and call it with any `.cypcb` source string — should return `{ snapshot, errors }`.
- Import `WorkerRequest`/`WorkerResponse` from `./worker-protocol` — TypeScript will enforce correct message shapes at compile time.

## Deviations

- Cleaned up unused type imports from `wasm.ts` (PadInfo, PinRef, BoardInfo, ComponentInfo, NetInfo) — not in the plan but necessary to avoid lint warnings after the extraction removed their only usage sites.
- Symlinked `node_modules` and `pkg` directories from main repo to worktree to enable `tsc` and `vitest` — infrastructure detail, not a code deviation.

## Known Issues

- Pre-existing `TS6133: 'showVariants' is declared but its value is never read` in `main.ts:28` — not introduced by this task, will likely be resolved when T04 wires up variant routing.

## Files Created/Modified

- `viewer/src/worker-protocol.ts` — **new** — WorkerRequest/WorkerResponse discriminated union types for worker↔main messaging
- `viewer/src/parse-source.ts` — **new** — parseSource() and helpers extracted from wasm.ts
- `viewer/src/wasm.ts` — **modified** — removed parser code, added import from parse-source.ts, cleaned unused type imports
